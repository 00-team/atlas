
import json
import math
import random

with open('states.json') as f:
    DATA = json.load(f)


C = []


def avg_point(geom: list[float, float]):
    a, b, c = 0, 0, 0
    for x, y in geom:
        a += x
        b += y
        c += 1
    return (a / c, b / c)


for s in DATA:
    C.append({
        "type": "Feature",
        "properties": {
            "name": s['name'],
            "name_en": s['name_en'],
            "marker-color": "#f00"
        },
        "geometry": {
            "type": "Point",
            "coordinates": s['avg']
        }
    })
    C.append({
        "type": "Feature",
        "properties": {
            "name": s['name'],
            "name_en": s['name_en'],
            "marker-color": "#0f0"
        },
        "geometry": {
            "type": "Point",
            "coordinates": s['center']
        }
    })


with open('geojson-2.json', 'w') as f:
    json.dump({"type": "FeatureCollection", "features": C},
              f, indent=2, ensure_ascii=False)

    # if item['name'] != 'ایران':
    #     continue

    # G = item['states'][0]
    # break

# N = []
#
# print(G)
#
# for idx, c in enumerate(G[0]):
#     if idx % 10 != 0:
#         continue
#     N.append(c)
#
#
#

#
#
# with open('gg.json', 'w') as f:
#     json.dump(fc, f)
#

# [
#   [ 48.2924209, 36.5937064 ],
#   [ 47.7056958, 35.7317088 ],
#   [ 49.4008049, 34.9239588 ],
#   [ 50.7899655, 35.8966793 ],
#   [ 48.2924209, 36.5937064 ]
#
