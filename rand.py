import random
import json


with open('eats.json') as f:
    DATA = json.load(f)

random.shuffle(DATA)


with open('eats-r.json', 'w') as f:
    json.dump(DATA, f, ensure_ascii=False)
