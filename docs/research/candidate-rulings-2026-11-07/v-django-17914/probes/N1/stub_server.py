"""A minimal PostgreSQL wire-protocol stub, written for this probe.

It is not PostgreSQL. It accepts any login, reports server_version 16.0 and
TimeZone UTC, answers every SELECT with one int4 row holding 1, and answers
any other statement with a command tag. `restart()` ends every open session
the way a server shutdown does: it sends FATAL 57P01 "terminating connection
due to administrator command" and closes the socket, then keeps accepting new
sessions. `drop()` closes every open socket without a message, the way a
proxy or load balancer that resets idle connections does.
"""
import itertools
import socket
import struct
import threading


def _msg(kind, payload=b""):
    return kind + struct.pack("!i", len(payload) + 4) + payload


def _cstr(s):
    return s.encode() + b"\0"


class StubServer:
    def __init__(self):
        self.sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self.sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        self.sock.bind(("127.0.0.1", 0))
        self.sock.listen(64)
        self.port = self.sock.getsockname()[1]
        self.sessions = {}
        self.lock = threading.Lock()
        self.pids = itertools.count(1000)
        self.opened = 0
        threading.Thread(target=self._accept, daemon=True).start()

    def live_sessions(self):
        with self.lock:
            return len(self.sessions)

    def restart(self):
        return self._end_all(message=True)

    def drop(self):
        return self._end_all(message=False)

    def _end_all(self, message):
        with self.lock:
            conns = list(self.sessions.items())
            self.sessions.clear()
        fatal = _msg(
            b"E",
            b"SFATAL\0VFATAL\0C57P01\0"
            + b"Mterminating connection due to administrator command\0\0",
        )
        for pid, c in conns:
            try:
                if message:
                    c.sendall(fatal)
                c.shutdown(socket.SHUT_RDWR)
            except OSError:
                pass
            c.close()
        return len(conns)

    def _accept(self):
        while True:
            c, _ = self.sock.accept()
            threading.Thread(target=self._serve, args=(c,), daemon=True).start()

    def _recv(self, c, n):
        buf = b""
        while len(buf) < n:
            chunk = c.recv(n - len(buf))
            if not chunk:
                raise ConnectionError
            buf += chunk
        return buf

    def _serve(self, c):
        pid = next(self.pids)
        try:
            while True:
                (length,) = struct.unpack("!i", self._recv(c, 4))
                body = self._recv(c, length - 4)
                (code,) = struct.unpack("!i", body[:4])
                if code in (80877103, 80877104):  # SSLRequest, GSSENCRequest
                    c.sendall(b"N")
                    continue
                if code == 80877102:  # CancelRequest
                    c.close()
                    return
                break
            with self.lock:
                self.sessions[pid] = c
                self.opened += 1
            out = _msg(b"R", struct.pack("!i", 0))
            for k, v in (
                ("server_version", "16.0"),
                ("server_encoding", "UTF8"),
                ("client_encoding", "UTF8"),
                ("DateStyle", "ISO, MDY"),
                ("integer_datetimes", "on"),
                ("standard_conforming_strings", "on"),
                ("TimeZone", "UTC"),
                ("IntervalStyle", "postgres"),
                ("is_superuser", "on"),
                ("session_authorization", "stub"),
            ):
                out += _msg(b"S", _cstr(k) + _cstr(v))
            out += _msg(b"K", struct.pack("!ii", pid, 1))
            status = b"I"
            out += _msg(b"Z", status)
            c.sendall(out)
            portal_sql = ""
            while True:
                kind = self._recv(c, 1)
                (length,) = struct.unpack("!i", self._recv(c, 4))
                body = self._recv(c, length - 4)
                if kind == b"X":
                    return
                if kind == b"Q":
                    sql = body.rstrip(b"\0").decode()
                    out, status = self._run(sql, status, describe=True)
                    c.sendall(out + _msg(b"Z", status))
                elif kind == b"P":
                    name_end = body.index(b"\0")
                    sql_end = body.index(b"\0", name_end + 1)
                    portal_sql = body[name_end + 1 : sql_end].decode()
                    c.sendall(_msg(b"1"))
                elif kind == b"B":
                    c.sendall(_msg(b"2"))
                elif kind == b"D":
                    if body[:1] == b"S":
                        c.sendall(_msg(b"t", struct.pack("!h", 0)))
                    c.sendall(self._describe(portal_sql))
                elif kind == b"E":
                    out, status = self._run(portal_sql, status, describe=False)
                    c.sendall(out)
                elif kind == b"S":
                    c.sendall(_msg(b"Z", status))
                elif kind == b"C":
                    c.sendall(_msg(b"3"))
                elif kind == b"H":
                    pass
        except (ConnectionError, OSError):
            pass
        finally:
            with self.lock:
                self.sessions.pop(pid, None)
            try:
                c.close()
            except OSError:
                pass

    @staticmethod
    def _rowdesc():
        field = _cstr("?column?") + struct.pack("!ihihih", 0, 0, 23, 4, -1, 0)
        return _msg(b"T", struct.pack("!h", 1) + field)

    def _describe(self, sql):
        if sql.strip().upper().startswith("SELECT"):
            return self._rowdesc()
        return _msg(b"n")

    def _run(self, sql, status, describe):
        word = (sql.strip().split() or [""])[0].upper()
        if not word:
            return _msg(b"I"), status
        if word == "SELECT":
            out = self._rowdesc() if describe else b""
            out += _msg(b"D", struct.pack("!hi", 1, 1) + b"1")
            out += _msg(b"C", _cstr("SELECT 1"))
            return out, status
        if word in ("BEGIN", "START"):
            status = b"T"
        elif word in ("COMMIT", "ROLLBACK", "END"):
            status = b"I"
        return _msg(b"C", _cstr(word)), status
