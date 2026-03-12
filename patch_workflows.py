import re

# Wait, if node20 action is deprecated, maybe the github actions used in ci.yml are too old?
# Let's check checkout version
with open('.github/workflows/ci.yml') as f:
    print(f.read())
