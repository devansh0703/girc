import urllib.request
for path in ("/health", "/", "/login", "/register"):
    r = urllib.request.urlopen("http://127.0.0.1:8000" + path)
    body = r.read()
    print(path, r.status, len(body), "bytes, html:", b"<html" in body.lower())
