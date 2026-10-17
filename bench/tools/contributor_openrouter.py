"""Capture versioned OpenRouter client records and apply frozen token prices."""
from __future__ import annotations
import math
from pathlib import Path
import sqlite3
import json
from contributor_common import Refused


def open_code(attempt, rows):
    path = attempt / 'home/.local/share/opencode/opencode.db'
    database = sqlite3.connect(f'file:{path}?mode=ro', uri=True)
    try:
        for table in ('credential', 'account', 'control_account'):
            if database.execute(f'SELECT count(*) FROM {table}').fetchone()[0]:
                raise Refused('OpenCode saved credential state; it cannot enter contribution evidence')
        totals = {row[0]: row[1:] for row in database.execute('SELECT id,tokens_input,tokens_output,tokens_reasoning,tokens_cache_read,tokens_cache_write FROM session_v2')}
        sessions = [{'id': row[0], 'parent': row[1], 'fork': row[2], 'outcome': row[3]}
                    for row in database.execute('SELECT id,parent_id,fork_session_id,idle_outcome FROM session_v2')]
        roots = [row for row in sessions if row['parent'] is None]
        if len(roots) != 1 or any(row['fork'] for row in sessions):
            raise Refused('OpenCode needs exactly one fresh root session and no history forks')
        by_id = {row['id']: row for row in sessions}
        for session in sessions:
            visited = set()
            current = session
            while current['parent']:
                if current['id'] in visited or current['parent'] not in by_id:
                    raise Refused('OpenCode has an unrecorded or cyclic session parent')
                visited.add(current['id'])
                current = by_id[current['parent']]
        messages = [{'id': row[0], 'session': row[1], **json.loads(row[2])}
                    for row in database.execute("SELECT id,session_id,data FROM session_message WHERE type='assistant' ORDER BY time_created,id")]
    finally:
        database.close()
    details = [{'model': m.get('model', {}).get('id'), 'provider': m.get('model', {}).get('providerID'),
                'effort': m.get('model', {}).get('variant')} for m in messages]
    requests = []
    for message in messages:
        tokens = message.get('tokens', {})
        requests.append({'message': message, 'input': tokens.get('input'),
                         'output': (tokens['output'] + tokens['reasoning']) if 'output' in tokens and 'reasoning' in tokens else None,
                         'cache_read': tokens.get('cache', {}).get('read'), 'cache_write': tokens.get('cache', {}).get('write')})
    starts = [r for r in rows if r.get('type') == 'step_start']
    root_messages = [m for m in messages if m['session'] == roots[0]['id']]
    ids = [r.get('messageID', r.get('part', {}).get('messageID')) for r in starts]
    stream_complete = len(starts) == len(root_messages) and len(set(ids)) == len(ids) and not any(r.get('type') == 'error' for r in rows)
    sums_complete = True
    for session in sessions:
        accumulated = [0, 0, 0, 0, 0]
        for message in messages:
            if message['session'] != session['id']:
                continue
            tokens = message.get('tokens', {})
            counts = [tokens.get(key) for key in ('input', 'output', 'reasoning')]
            counts += [tokens.get('cache', {}).get(key) for key in ('read', 'write')]
            if any(isinstance(v, bool) or not isinstance(v, int) or v < 0 for v in counts):
                sums_complete = False
                continue
            accumulated = [a + b for a, b in zip(accumulated, counts)]
        sums_complete = sums_complete and tuple(accumulated) == totals[session['id']]
    complete = sums_complete and stream_complete and bool(messages) and all(row['outcome'] == 'succeeded' for row in sessions)
    complete = complete and all(m.get('time', {}).get('completed') and m.get('finish') in ('stop', 'tool-calls') for m in messages)
    return requests, details, complete, {'sessions': sessions}


def pi(rows):
    messages = [row['message'] for row in rows if row.get('type') == 'message_end' and row.get('message', {}).get('role') == 'assistant']
    details = [{'model': m.get('model'), 'provider': m.get('provider'), 'effort': m.get('thinkingLevel')} for m in messages]
    requests = []
    for message in messages:
        usage = message.get('usage', {})
        requests.append({'message': message, 'input': usage.get('input'), 'output': usage.get('output'),
                         'cache_read': usage.get('cacheRead'), 'cache_write': usage.get('cacheWrite')})
    complete = bool(messages) and any(r.get('type') == 'agent_settled' and r.get('aborted') is False for r in rows)
    complete = complete and messages[-1].get('stopReason') == 'stop'
    complete = complete and all(m.get('stopReason') in ('stop', 'toolUse') for m in messages)
    return requests, details, complete, {}


def price(requests, pricing, complete):
    usage = {'metering_status': 'incomplete', 'priced_total_usd': None, 'basis': 'frozen OpenRouter prices per token', 'pricing': pricing}
    if not complete:
        usage['reason'] = 'client did not record a successful complete request sequence'
        return usage
    total = 0.0
    for request in requests:
        counts = {key: request.get(key) for key in ('input', 'output', 'cache_read', 'cache_write')}
        if any(isinstance(value, bool) or not isinstance(value, int) or value < 0 for value in counts.values()) or sum(counts.values()) == 0:
            usage['reason'] = 'missing, invalid, or zero-default request token usage'
            return usage
        for count, field in [('input', 'prompt'), ('output', 'completion'), ('cache_read', 'input_cache_read'), ('cache_write', 'input_cache_write')]:
            if counts[count] == 0:
                continue
            try:
                rate = float(pricing[field])
            except (KeyError, ValueError, TypeError):
                usage['reason'] = f'frozen pricing has no usable {field} rate'
                return usage
            if not math.isfinite(rate) or rate < 0:
                usage['reason'] = f'frozen pricing has an invalid {field} rate'
                return usage
            total += counts[count] * rate
    usage.update(metering_status='complete', priced_total_usd=total, requests=len(requests))
    return usage
