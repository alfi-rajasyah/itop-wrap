#!/usr/bin/env python3
"""Read-only probe for Task 5c: how can iTop Wrap get pasted images and attachments?

It finds one of YOUR open tickets whose description or log has a pasted image, then tries:
  A. downloading the image link with HTTP Basic auth (only on your iTop's own host)
  B. reading the InlineImage object through REST (core/get)
and reads that ticket's attachments through REST.

It prints and saves ONLY a summary: status codes, content types, byte counts and counts.
No image bytes, descriptions, log text or file names are saved or printed.

Usage:
  python3 -I scripts/probe-images.py            # probe your server
  python3 -I scripts/probe-images.py --selftest # offline checks, no network
"""
import base64
import getpass
import json
import os
import re
import ssl
import sys
import urllib.error
import urllib.parse
import urllib.request

READ_ONLY_OPS = {"core/get", "core/check_credentials"}
OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "Tests", "ITopKitTests", "Fixtures", "images-probe.json")
IMG_SRC = re.compile(r'<img\b[^>]*?\bsrc="([^"]+)"', re.I)
DATA_ID = re.compile(r'\bdata-img-id="(\d+)"', re.I)


def image_refs(html):
    """(url, id, secret) for every <img> in the HTML."""
    out = []
    for tag in re.findall(r"<img\b[^>]*>", html or "", re.I):
        src = (IMG_SRC.search(tag) or [None, ""])[1].replace("&amp;", "&")
        q = urllib.parse.parse_qs(urllib.parse.urlparse(src).query)
        img_id = (DATA_ID.search(tag) or [None, None])[1] or (q.get("id") or [None])[0]
        out.append((src, img_id, (q.get("s") or [None])[0]))
    return out


class Client:
    def __init__(self, base, user, pwd):
        self.base = base.rstrip("/")
        self.host = urllib.parse.urlparse(self.base).hostname
        self.user, self.pwd = user, pwd
        self.ctx = ssl.create_default_context()

    def call(self, payload):
        if payload.get("operation") not in READ_ONLY_OPS:
            raise SystemExit("Refused: %r is not a read operation." % payload.get("operation"))
        body = urllib.parse.urlencode({"auth_user": self.user, "auth_pwd": self.pwd, "json_data": json.dumps(payload)}).encode()
        req = urllib.request.Request(self.base + "/webservices/rest.php?version=1.4", data=body, method="POST")
        with urllib.request.urlopen(req, context=self.ctx, timeout=60) as r:
            return json.loads(r.read().decode("utf-8"))

    def get(self, cls, key, fields, limit=None):
        p = {"operation": "core/get", "class": cls, "key": key, "output_fields": fields}
        if limit:
            p["limit"] = str(limit)
        return self.call(p)

    def fetch_basic(self, url):
        """GET with HTTP Basic auth; only on the iTop host. Returns a summary, never the bytes."""
        full = urllib.parse.urljoin(self.base + "/", url)
        if urllib.parse.urlparse(full).hostname != self.host:
            return {"skipped": "other host"}
        token = base64.b64encode(("%s:%s" % (self.user, self.pwd)).encode()).decode()
        req = urllib.request.Request(full, headers={"Authorization": "Basic " + token})
        try:
            with urllib.request.urlopen(req, context=self.ctx, timeout=30) as r:
                data = r.read()
                return {"status": r.status, "content_type": r.headers.get("Content-Type", ""), "bytes": len(data),
                        "looks_like_login_page": b"<html" in data[:2000].lower()}
        except urllib.error.HTTPError as e:
            return {"status": e.code, "content_type": e.headers.get("Content-Type", "")}


def first(resp):
    return next(iter((resp.get("objects") or {}).values()), None)


def probe():
    print("iTop Wrap image probe. Read-only; saves only a summary.\n", file=sys.stderr)
    base = input("Server URL (https://...): ").strip()
    user = input("Username: ").strip()
    pwd = getpass.getpass("Password (hidden): ")
    c = Client(base, user, pwd)
    summary = {"notes": []}

    me = first(c.get("User", "SELECT User WHERE login = '%s'" % user.replace("'", "\\'"), "contactid", 1))
    contact = int((me or {}).get("fields", {}).get("contactid", 0) or 0)

    ticket, where, refs = None, None, []
    for cls in ("UserRequest", "Incident"):
        for field in ("description", "public_log"):
            oql = "SELECT %s WHERE agent_id = %d AND %s LIKE '%%<img%%'" % (cls, contact, field)
            r = c.get(cls, oql, "description,public_log,private_log", 5)
            summary["notes"].append("%s with <img> in %s: %s found" % (cls, field, r.get("message", "?")))
            for o in (r.get("objects") or {}).values():
                f = o.get("fields", {})
                html = f.get("description", "") + " ".join(e.get("message_html", "") for e in (f.get("public_log") or {}).get("entries", []))
                found = [x for x in image_refs(html) if x[1]]
                if found:
                    ticket, where, refs = o, field, found
                    break
            if ticket:
                break
        if ticket:
            break

    if not ticket:
        summary["notes"].append("No ticket of yours with a pasted image was found.")
    else:
        cls, key = ticket["class"], ticket["key"]
        hosts = {urllib.parse.urlparse(urllib.parse.urljoin(c.base + "/", u)).hostname for u, _, _ in refs}
        summary["ticket"] = {"class": cls, "images": len(refs), "found_in": where,
                             "image_hosts": ["your iTop" if h == c.host else "other host" for h in hosts],
                             "src_shape": sorted({re.sub(r"=[^&]*", "=…", urllib.parse.urlparse(u).path + "?" + urllib.parse.urlparse(u).query) for u, _, _ in refs})}
        url, img_id, secret = refs[0]
        summary["A_basic_auth_download"] = c.fetch_basic(url)
        r = c.get("InlineImage", "SELECT InlineImage WHERE id = %d" % int(img_id), "contents,item_class,item_id", 1)
        o = first(r)
        contents = (o or {}).get("fields", {}).get("contents") or {}
        summary["B_rest_inlineimage"] = {"code": r.get("code"), "message": r.get("message"), "returned": bool(o),
                                         "has_data": bool(contents.get("data")), "mimetype": contents.get("mimetype"),
                                         "base64_chars": len(contents.get("data") or ""),
                                         "fields": sorted(((o or {}).get("fields") or {}).keys())}
        a = c.get("Attachment", "SELECT Attachment WHERE item_class = '%s' AND item_id = %d" % (cls, int(key)), "contents", 20)
        att = list((a.get("objects") or {}).values())
        summary["attachments"] = {"code": a.get("code"), "message": a.get("message"), "count": len(att),
                                  "fields": sorted((att[0].get("fields") or {}).keys()) if att else [],
                                  "contents_keys": sorted(((att[0].get("fields") or {}).get("contents") or {}).keys()) if att else [],
                                  "sizes": [len(((x.get("fields") or {}).get("contents") or {}).get("data") or "") for x in att]}

    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    with open(OUT, "w") as f:
        json.dump(summary, f, indent=2)
        f.write("\n")
    print(json.dumps(summary, indent=2))
    print("\nSaved the summary to %s. Only read operations were sent." % os.path.relpath(OUT), file=sys.stderr)


def selftest():
    html = ('<p>x</p><img src="https://itop.example.com/pages/ajax.document.php?operation=download_inlineimage&amp;id=42&amp;s=abc" '
            'data-img-id="42" data-img-secret="abc"><img src="https://cdn.other.example/a.png">')
    refs = image_refs(html)
    assert refs[0][1] == "42" and refs[0][2] == "abc", refs
    assert refs[1][1] is None
    c = Client("https://itop.example.com", "u", "p")
    assert c.fetch_basic("https://cdn.other.example/a.png") == {"skipped": "other host"}
    try:
        c.call({"operation": "core/update"})
        raise AssertionError("write not refused")
    except SystemExit as e:
        assert "Refused" in str(e)
    print("selftest passed: image refs parsed, other hosts skipped, writes refused")


if __name__ == "__main__":
    sys.exit(selftest() if "--selftest" in sys.argv else probe())
