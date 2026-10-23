# Traffic territory outlines

`traffic-territories.json` contains only Hong Kong, Macau and the US Virgin Islands from
[`country-coder` borders.json](https://github.com/rapideditor/country-coder/blob/c30d4314809cb8a446c3d42181cd370a1a424056/src/data/borders.json),
revision `c30d4314809cb8a446c3d42181cd370a1a424056` (retrieved 2026-10-23).
Only `iso1A2` and the unchanged MultiPolygon coordinates are retained, as `code` and `polygons`.
Coordinates are longitude/latitude, with closed outer rings followed by holes.

These territories keep left while the country reported by OpenMapTiles keeps right.
The outlines are simplified location classifiers, not surveyed road boundaries.
No country-coder library or implementation code is included. The data is ISC-licensed;
the upstream copyright and permission notice is in `country-coder-LICENSE.md`.
