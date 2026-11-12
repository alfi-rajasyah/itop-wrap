#!/usr/bin/env python3
"""Read-only probe for images in log entries: how does this iTop store an image pasted in a web log entry?

It looks for log entries (public or private) with a pasted image on tickets you can see, then reads:
  - the shape of the <img> markup in the entry's HTML (attribute names only, never values)
  - the InlineImage behind it (field names, whether it points at the ticket, whether its secret matches the markup)
  - which fields an Attachment has, and your profiles (names only), to judge whether you may create them

It prints and saves ONLY a summary: counts, field and attribute names, true/false checks and sizes.
No log text, image bytes, secrets, names, ids or file names are saved or printed.
Nothing is written to iTop: only core/get is sent.

Usage:
  python3 -I scripts/probe-log-images.py            # probe your server
  python3 -I scripts/probe-log-images.py --selftest # offline checks, no network
"""
import getpass
import json
import os
import re
import ssl
import sys
import urllib.parse
import urllib.request

READ_ONLY_OPS = {"core/get"}
OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "Tests", "ITopKitTests", "Fixtures", "log-images-probe.json")
ATTACHMENT_FIELDS = "item_class,item_id,item_org_id,temp_id,expire,creation_date,user_id,contact_id"
IMG_TAG = re.compile(r"<img\b[^>]*>", re.I)
ATTR = re.compile(r'\b([a-zA-Z_:][-a-zA-Z0-9_:.]*)\s*=\s*"([^"]*)"')


def img_tags(html):
    """[{attr: value}] for every <img> tag in the HTML."""
    return [dict((k.lower(), v.replace("&amp;", "&")) for k, v in ATTR.findall(tag)) for tag in IMG_TAG.findall(html or "")]


def src_shape(src):
    """The src with every value blanked: '/pages/ajax.document.php?operation=…&id=…&s=…'."""
    u = urllib.parse.urlparse(src)
    query = re.sub(r"=[^&]*", "=…", u.query)
    return u.path + ("?" + query if query else "")


def operation_of(src):
    return (urllib.parse.parse_qs(urllib.parse.urlparse(src).query).get("operation") or [None])[0]


class Client:
    def __init__(self, base, user, pwd):
        self.base = base.rstrip("/")
        self.user, self.pwd = user, pwd
        self.ctx = ssl.create_default_context()

    def call(self, payload):
        if payload.get("operation") not in READ_ONLY_OPS:
            raise SystemExit("Refused: %r is not a read operation." % payload.get("operation"))
        body = urllib.parse.urlencode({"auth_user": self.user, "auth_pwd": self.pwd, "json_data": json.dumps(payload)}).encode()
        req = urllib.request.Request(self.base + "/webservices/rest.php?version=1.4", data=body, method="POST")
        with urllib.request.urlopen(req, context=self.ctx, timeout=90) as r:
            return json.loads(r.read().decode("utf-8"))

    def get(self, cls, key, fields, limit=None):
        p = {"operation": "core/get", "class": cls, "key": key, "output_fields": fields}
        if limit:
            p["limit"] = str(limit)
        return self.call(p)


def objects(resp):
    return list((resp.get("objects") or {}).values())


def summarize_entry(entry):
    html = entry.get("message_html") or ""
    tags = img_tags(html)
    return {
        "entry_keys": sorted(entry.keys()),
        "images": len(tags),
        "img_attributes": sorted({k for t in tags for k in t}),
        "src_shapes": sorted({src_shape(t.get("src", "")) for t in tags}),
        "src_operations": sorted({operation_of(t.get("src", "")) or "none" for t in tags}),
        "src_is_absolute": sorted({t.get("src", "").startswith("http") for t in tags}),
        "has_data_img_id": all("data-img-id" in t for t in tags),
        "has_data_img_secret": all("data-img-secret" in t for t in tags),
        "style_values": sorted({re.sub(r"\d+", "N", t.get("style", "")) for t in tags if t.get("style")}),
        "wrapped_in_p": bool(re.search(r"<p>\s*<img", html, re.I)),
        "plain_message_mentions_image": bool(re.search(r"img|image", entry.get("message") or "", re.I)),
    }


def probe():
    print("iTop Wrap log image probe. Read-only; saves only a summary.\n", file=sys.stderr)
    base = input("Server URL (https://...): ").strip()
    user = input("Username: ").strip()
    pwd = getpass.getpass("Password (hidden): ")
    c = Client(base, user, pwd)
    s = {"notes": []}

    # Log entries with a pasted image.
    found = []  # (ticket class, ticket key, ticket org, entry, tags)
    for cls in ("UserRequest", "Incident"):
        for field in ("public_log", "private_log"):
            oql = "SELECT %s WHERE %s LIKE '%%<img%%'" % (cls, field)
            r = c.get(cls, oql, "%s,org_id" % field, 10)
            obs = objects(r)
            s["notes"].append("%s.%s with <img>: code %s, %d returned" % (cls, field, r.get("code"), len(obs)))
            for o in obs:
                for e in ((o.get("fields") or {}).get(field) or {}).get("entries", []):
                    tags = img_tags(e.get("message_html"))
                    if tags:
                        found.append((o["class"], o["key"], (o.get("fields") or {}).get("org_id"), field, e, tags))
    s["log_entries_with_images"] = len(found)
    s["entries"] = [dict(summarize_entry(e), log=f) for _, _, _, f, e, _ in found[:5]]

    # The InlineImage behind each image (up to 5).
    images = []
    for cls, key, org, _, _, tags in found:
        for t in tags:
            img_id = t.get("data-img-id") or (urllib.parse.parse_qs(urllib.parse.urlparse(t.get("src", "")).query).get("id") or [None])[0]
            if not img_id or not img_id.isdigit() or len(images) >= 5:
                continue
            r = c.get("InlineImage", "SELECT InlineImage WHERE id = %d" % int(img_id), "*", 1)
            o = (objects(r) or [None])[0]
            f = (o or {}).get("fields") or {}
            contents = f.get("contents") or {}
            src_secret = (urllib.parse.parse_qs(urllib.parse.urlparse(t.get("src", "")).query).get("s") or [None])[0]
            images.append({
                "code": r.get("code"), "returned": bool(o),
                "fields": sorted(f.keys()),
                "contents_keys": sorted(contents.keys()),
                "mimetype": contents.get("mimetype"),
                "kib": round(len(contents.get("data") or "") * 3 / 4 / 1024),
                "item_class_is_ticket_class": f.get("item_class") == cls,
                "item_id_is_ticket": str(f.get("item_id")) == str(key),
                "item_org_id_is_ticket_org": str(f.get("item_org_id")) == str(org),
                "secret_matches_data_img_secret": bool(f.get("secret")) and f.get("secret") == t.get("data-img-secret"),
                "secret_matches_src_s": bool(f.get("secret")) and f.get("secret") == src_secret,
                "secret_shape": re.sub(r"[0-9a-f]", "h", f.get("secret") or ""),
                "temp_id_empty": not f.get("temp_id"),
                "expire_set": bool(f.get("expire")),
            })
    s["inline_images"] = images

    # Attachments on tickets (small fields only: asking for contents of every Attachment times out).
    try:
        a = c.get("Attachment", "SELECT Attachment WHERE item_class IN ('UserRequest', 'Incident')", ATTACHMENT_FIELDS, 3)
        att = objects(a)
        s["attachments"] = {"code": a.get("code"), "message": re.sub(r"\d+", "N", a.get("message") or ""), "returned": len(att),
                            "fields": sorted(((att[0].get("fields") or {}).keys())) if att else [],
                            "temp_id_empty": [not (x.get("fields") or {}).get("temp_id") for x in att],
                            "expire_set": [bool((x.get("fields") or {}).get("expire")) for x in att]}
    except Exception as err:
        s["attachments"] = {"error": type(err).__name__}

    # Your profiles: who may create InlineImage and Attachment is decided by them.
    try:
        me = (objects(c.get("User", "SELECT User WHERE login = '%s'" % user.replace("\\", "\\\\").replace("'", "\\'"), "id", 1)) or [None])[0]
        if me:
            p = c.get("URP_UserProfile", "SELECT URP_UserProfile WHERE userid = %d" % int(me["key"]), "profile", 20)
            s["profiles"] = sorted({(x.get("fields") or {}).get("profile") for x in objects(p)} - {None})
        else:
            s["notes"].append("Could not read your User record.")
    except Exception as err:
        s["profiles_error"] = type(err).__name__

    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    with open(OUT, "w") as f:
        json.dump(s, f, indent=2)
        f.write("\n")
    print(json.dumps(s, indent=2))
    print("\nSaved the summary to %s. Only core/get was sent." % os.path.relpath(OUT), file=sys.stderr)


def selftest():
    html = ('<p>see</p><p><img src="https://itop.example.com/pages/ajax.document.php?operation=download_inlineimage&amp;id=42&amp;s=ab12" '
            'style="width: 300px;" data-img-id="42" data-img-secret="ab12"></p>')
    tags = img_tags(html)
    assert tags[0]["data-img-id"] == "42" and tags[0]["data-img-secret"] == "ab12", tags
    assert src_shape(tags[0]["src"]) == "/pages/ajax.document.php?operation=…&id=…&s=…"
    assert operation_of(tags[0]["src"]) == "download_inlineimage"
    e = summarize_entry({"message": "see", "message_html": html, "date": "x"})
    assert e["has_data_img_id"] and e["wrapped_in_p"] and e["style_values"] == ["width: Npx;"], e
    assert "42" not in json.dumps(e) and "ab12" not in json.dumps(e), "summary leaks values"
    c = Client("https://itop.example.com", "u", "p")
    try:
        c.call({"operation": "core/create"})
        raise AssertionError("write not refused")
    except SystemExit as err:
        assert "Refused" in str(err)
    print("selftest passed: markup parsed, values kept out of the summary, writes refused")


if __name__ == "__main__":
    sys.exit(selftest() if "--selftest" in sys.argv else probe())
