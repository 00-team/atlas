import json
import math
import random

with open('sector-db.json') as f:
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


def poly_to_geom(mp: list) -> list:
    out = []
    for p in mp:
        poly = []
        for ex in p['exterior']:
            x, y = ex['x'], ex['y']
            poly.append([x, y])
        out.append([poly])

    return out


N = []

for n in DATA['nations'].values():
    N.append({
        "type": "Feature",
        "properties": {
            "name": n['name'],
            "id": n['id'],
            "index": n['index'],
            "stroke": "#ff0"
            },
        "geometry": {
            "type": "MultiPolygon",
            "coordinates": poly_to_geom(n['poly'])
            }
    })

for n in DATA['nations'].values():
    print(n['name'], '|', n['id'])
    for r in n['regions'].values():
        print('   ', r['name'], '|', r['id'])
        # for c in r['cantons'].values():
        #     print('       ', c['name'], '|', c['id'])
        x = r
        C.append({
                "type": "Feature",
                "properties": {
                    "name": x['name'],
                    "id": x['id'],
                    "index": x['index'],
                    # "stroke": "#ff0"
                },
                "geometry": {
                    "type": "MultiPolygon",
                    "coordinates": poly_to_geom(x['poly'])
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
        "features": [*N, *C]
    }, f,
        indent=2,
        ensure_ascii=False
    )
