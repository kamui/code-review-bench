import socket, threading, sys, requests
srv = socket.socket(); srv.bind(("127.0.0.1", 0)); srv.listen(5); port = srv.getsockname()[1]
def acc():
    while True:
        try:
            c,_ = srv.accept()
            threading.Thread(target=lambda c=c: (__import__('time').sleep(3), c.close()), daemon=True).start()
        except Exception: return
threading.Thread(target=acc, daemon=True).start()
try:
    requests.get("http://plain.example/", proxies={"http": f"https://127.0.0.1:{port}"}, timeout=1.5)
    print(sys.argv[1], "no exception")
except BaseException as e:
    print(sys.argv[1], "requests.get raised:", type(e).__module__ + "." + type(e).__name__, "-", str(e)[:120])
