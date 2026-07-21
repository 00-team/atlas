import json
import math
import random

with open('sectors.json') as f:
    DATA = json.load(f)


def gn(item: dict) -> str:
    ne: str = item['name_en']
    if ne is not None:
        return ne.lower().replace(' ', '_').replace('_province', '').replace('_county', '')
    return item['name']


def simp(geom: list) -> list:
    return geom

    new = [geom[0]]
    for idx, (x, y) in enumerate(geom[1:-1]):
        if idx % 2 != 0:
            continue

        new.append([x, y])

    new.append(geom[-1])
    return new


C = []

for cn in DATA.values():
    if cn['level'] != 3:
        continue

    # if len(cn['area']) == 1:
    #     continue

    C.append({
        "type": "Feature",
        "properties": {
                "name": cn['name'],
                "id": cn['id'],
                # "stroke": "#ff0"
        },
        "geometry": {
            "type": "MultiPolygon",
            "coordinates": cn['area']

        }
    })

    # for s in cn['states']:
    #     sn = gn(s)
    #     print(cnn, sn)
    #
    #     for a in s['area']:
    #         C.append({
    #             "type": "Feature",
    #             "properties": {
    #                 "name": s['name'],
    #                 "name_en": s['name_en'],
    #                 "stroke": "#ff0"
    #             },
    #             "geometry": {
    #                 "type": "Polygon",
    #                 "coordinates": [
    #                     simp(a['geom'][0])
    #                 ]
    #             }
    #         })

    # for c in s['cities']:
    #     print(cnn, sn, gn(c))
    #     for a in c['area']:
    #         C.append({
    #             "type": "Feature",
    #             "properties": {
    #                 "state": s['name'],
    #                 "name": c['name'],
    #                 "name_en": c['name_en'],
    #             },
    #             "geometry": {
    #                 "type": "Polygon",
    #                 "coordinates": [
    #                     simp(a['geom'][0])
    #                 ]
    #             }
    #         })


with open('geojson.json', 'w') as f:
    json.dump({
        "type": "FeatureCollection",
        "features": C
    }, f,
        indent=2,
        ensure_ascii=False
    )
