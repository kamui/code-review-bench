#!/usr/bin/env python3

import time
import uuid
import functools
import threading
import statistics
import requests
import sys
from concurrent import futures

MAX_DELAY = 1.0 #Seconds
BATCH_DIV = 10
BASE_URI = "tutamen.vrg1.aws.volaticus.net"
ACS_URI = "acs." + BASE_URI
SS_URI = "ss." + BASE_URI

# Note: This relies on synchronous 'authorizations post' behavior
# If 'authorizations post' is asynchronous, this needs to wait for completion
def get_ac_auth(path_crt, path_key, obj_perm, obj_type, obj_uid=None):

    url = "https://" + ACS_URI + "/api/v1/authorizations/"

    json = {'objperm': obj_perm,
            'objtype': obj_type,
            'objuid': str(obj_uid) if obj_uid else ""}

    res = requests.post(url=url, json=json, cert=(path_crt, path_key))
    res.raise_for_status()
    uid = res.json()['authorizations'][0]
    return uid

def get_ac_token(path_crt, path_key, uid):

    url = "https://" + ACS_URI + "/api/v1/authorizations/" + str(uid) + "/"

    res = requests.get(url=url, cert=(path_crt, path_key))
    res.raise_for_status()
    authz = res.json()
    return authz['token']

def get_ss_secret(token, col_uid, sec_uid):

    url = "https://" + SS_URI + "/api/v1/collections/" + str(col_uid) + "/secrets/" + str(sec_uid) + "/versions/latest/"

    header = {'tutamen-tokens': token}
    res = requests.get(url=url, headers=header)
    res.raise_for_status()
    authz = res.json()
    return authz['data']

def get_ac_null_cert(path_crt, path_key):

    url = "https://" + ACS_URI + "/api/v1/"
    res = requests.get(url=url, cert=(path_crt, path_key))
    res.raise_for_status()

def get_ac_null():

    url = "https://" + ACS_URI + "/api/v1/"
    res = requests.get(url=url)
    res.raise_for_status()

def get_ac_https():

    url = "https://" + ACS_URI + "/"
    res = requests.get(url=url)
    res.raise_for_status()

def get_ac_http():

    url = "http://" + ACS_URI + "/"
    res = requests.get(url=url)
    res.raise_for_status()

def get_ss_null():

    url = "https://" + SS_URI + "/api/v1/"
    res = requests.get(url=url)
    res.raise_for_status()

def get_ss_https():

    url = "https://" + SS_URI + "/"
    res = requests.get(url=url)
    res.raise_for_status()

def get_ss_http():

    url = "http://" + SS_URI + "/"
    res = requests.get(url=url)
    res.raise_for_status()

def res_time():

    def _decorator(func):

        @functools.wraps(func)
        def _wrapper(*args, **kwargs):

            start = time.time()
            func(*args, **kwargs)
            dur = time.time() - start
            return dur

        return _wrapper

    return _decorator

def target_iops(iops_target, cnt, function, *args, **kwargs):

    @res_time()
    def timed_function(*timed_args, **timed_kwargs):
        try:
            function(*timed_args, **timed_kwargs)
        except Exception as error:
            print(error)

    batch_size = int(cnt // BATCH_DIV)
    threads = int(iops_target * MAX_DELAY)
    pause = float(batch_size) / float(iops_target)
    rounds = int(cnt // batch_size)

    futrs = []
    times = []
    start = time.time()

    with futures.ThreadPoolExecutor(max_workers=threads) as e:

        rnd = 0
        while True:
            for k in range(0, batch_size):
                futrs.append(e.submit(timed_function, *args, **kwargs))
            rnd += 1
            run_t_actual = time.time() - start
            run_t_target = rnd * pause
            if run_t_actual < run_t_target:
                time.sleep(run_t_target - run_t_actual)
            if rnd >= rounds:
                break

        for f in futrs:
            times.append(f.result())

    tot = time.time() - start
    return tot, times

def benchmark(iops_start, iops_end, iops_step, cnt, function, *args, **kwargs):

    #Precook
#    print("Precooking...")
    target_iops(iops_start, cnt, function, *args, **kwargs)

#    print("Benchmarking...")
#    print("   cnt  |  time  |  trgt  |  iops  | latavg | latstd ")
    for tgt in range(iops_start, iops_end, iops_step):

        tot, times = target_iops(tgt, cnt, function, *args, **kwargs)
        cnt = len(times)
        iops = (float(cnt) / float(tot))
        avg = statistics.mean(times)
        std = statistics.pstdev(times)
        print("  {:4d}     {:4.1f}     {:4d}   {:6.1f}    {:6.3f}   {:6.3f} ".format(cnt, tot, tgt,
                                                                                     iops, avg, std))

if __name__ == "__main__":

    path_crt = sys.argv[1]
    path_key = sys.argv[2]
    iops_start = int(sys.argv[3])
    iops_end = int(sys.argv[4])
    iops_step = int(sys.argv[5])
    cnt = int(sys.argv[6])
    test = sys.argv[7]

    if test == "get_ss_null":
        benchmark(iops_start, iops_end, iops_step, cnt,
                  get_ss_null)

    elif test == "get_ac_null":
        benchmark(iops_start, iops_end, iops_step, cnt,
                  get_ac_null)

    elif test == "get_ac_null_cert":
        benchmark(iops_start, iops_end, iops_step, cnt,
                  get_ac_null_cert, path_crt, path_key)

    elif test == "get_ac_https":
        benchmark(iops_start, iops_end, iops_step, cnt,
                  get_ac_https)

    elif test == "get_ss_https":
        benchmark(iops_start, iops_end, iops_step, cnt,
                  get_ss_https)

    elif test == "get_ac_http":
        benchmark(iops_start, iops_end, iops_step, cnt,
                  get_ac_http)

    elif test == "get_ss_http":
        benchmark(iops_start, iops_end, iops_step, cnt,
                  get_ss_http)

    elif test == "get_ac_auth":
        benchmark(iops_start, iops_end, iops_step, cnt,
                  get_ac_auth, path_crt, path_key, "create", "storageserver")

    elif test == "get_ss_secret":

        col_uid = uuid.UUID(sys.argv[8])
        sec_uid = uuid.UUID(sys.argv[9])

        auth_uid = get_ac_auth(path_crt, path_key,
                               "read", "collection", col_uid)
        token = get_ac_token(path_crt, path_key, auth_uid)

        benchmark(iops_start, iops_end, iops_step, cnt,
                  get_ss_secret, token, col_uid, sec_uid)

    else:
        print("Unrecognized test case")
