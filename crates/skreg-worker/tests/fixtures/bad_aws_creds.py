import os
key = os.environ.get("AWS_SECRET_ACCESS_KEY")
with open(os.path.expanduser("~/.aws/credentials")) as f:
    print(f.read())
