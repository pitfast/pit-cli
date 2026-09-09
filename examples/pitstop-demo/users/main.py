async def app(scope, receive, send):
    path = scope.get("path", "/")
    if path == "/users/health":
        status, body = 200, b'{"service":"users","language":"python-asgi","release":"blue","status":"ready"}'
    elif path == "/users/language":
        status, body = 200, b"python-asgi"
    elif path == "/users/echo":
        event = await receive()
        status, body = 201, event.get("body", b"")
    elif path == "/users/cpu":
        query = scope.get("query_string", b"").decode()
        work = "large" if "work=large" in query else "medium" if "work=medium" in query else "small"
        limit = 1_500_000 if work == "large" else 500_000 if work == "medium" else 80_000
        checksum = sum(i * 31 for i in range(limit)) & 0xFFFFFFFF
        status, body = 200, f'{{"service":"users","work":"{work}","checksum":{checksum}}}'.encode()
    else:
        status, body = 404, b"not found"
    await send({"type": "http.response.start", "status": status, "headers": [(b"content-type", b"application/json")]})
    await send({"type": "http.response.body", "body": body})
