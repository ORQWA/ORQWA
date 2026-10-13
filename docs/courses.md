# Courses

A course is a prepared route saved as one `.tqc` file: the GPX track plus the terrain and map
data it needs. Courses ride **fully offline**, on any computer.

## Prepare a course

Import a GPX route with **Import** on the Courses tab while online. The track is put onto the
roads and paths it rides (from OpenStreetMap): GPS wander and corners cut between sparse points
disappear, so the road you ride is the real one. Where a service road, cycle path or side street
runs right beside the road, the route stays on the road unless the track clearly follows the
other. Where the file leaves the roads between two places on them — a planner's straight line
through a tunnel or a gallery, across a lake, or a GPS fix lost in a tunnel — the route
follows the roads between them, and their bridges and tunnels come with it. Only a stretch
much longer than the way by road (a real detour off the map) keeps its course.
The elevations come from a terrain model, checked against the file's own: where the model
shows a wall the road does not have (a ledge, a gallery's roof), the file's heights are used
there, and short spikes are cut.
Torqa asks for the course's **name** (suggested from the file), downloads terrain and map
data, builds the 3D world and adds the course to your library. If a course of that name exists
already, choose **Replace** to replace it or **Keep both**.
**Cancel** beside the loading bar stops preparing it (#219), for instance when it seems stuck;
what was downloaded so far stays on the computer, so importing it again picks up quickly. Should
preparing it fail unexpectedly, Torqa says so instead of loading forever.

## Find a course

The bar above the cards finds a course in a large library:

- **Search** shows the courses whose name, or the name of a country they pass, contains what
  you type, of any case.
- **Sort** orders them by name, length, climbing, estimated time, date added or the date you
  last rode them. Each order starts the natural way round (A to Z, short to long, newest and
  latest ridden first); the arrow beside it turns it round. Courses you never rode, or could not
  finish at the planned power, come last.
- **Group** puts them under headings by type, terrain, length or country, with how many each
  holds. A course that crosses borders is under each of its countries.
- The **chips** below show only some courses: **Type** (3D or video), **Terrain**, **Length**
  and, once your courses span two countries or more, **Country**. Chips of one kind add up
  (Hilly and Mountainous shows both), chips of different kinds narrow down (Hilly and
  25–50 km shows the hilly courses of that length).
  **Clear filters** turns them all off and empties the search.

| Terrain | Climbing per km |
|---|---|
| Flat | under 5 m |
| Rolling | 5–10 m |
| Hilly | 10–20 m |
| Mountainous | 20 m and more |

The length bands meet at 25, 50 and 100 km, or at 15, 30 and 60 miles with imperial units.

Each card's **Time** is about how long the course takes you at 75 % of your FTP, with your
weight, coasting the descents. It comes from the elevation profile the course file keeps for
its card, so it may differ by a few minutes from the course page's **Time at 75 % FTP**, which
rides the whole route.

Torqa remembers the order and grouping; the search and chips start empty each time.

### Countries

Each card lists the **countries** the course passes by their codes, e.g. CH · FR; hover over
them for their names. Torqa finds them on the OpenStreetMap map when the course is prepared:
which side of a border the route is on, a country counting once the route rides at least
200 m in it. The names are kept in the course file, so a course you share names its countries
offline too.

The map data does not name the country on every side of every border: in 2026 it often leaves
out Germany, Austria and Poland. A stretch of route beside such a border is in no known
country, and a course known nowhere sits under **No country**, as do videos without a place
(Tacx RLV). Courses prepared by an earlier Torqa, or offline, learn their countries in the
background the next time Torqa starts online; this rewrites their course files once.

## Ride a course

Click its card on the Courses tab, then **Ride**. No internet connection is needed. **← Courses**
or a click on the Courses tab brings the gallery back.

## Share a course

Course files are ordinary files: send them by mail, put them on Nextcloud, a USB stick or a
website. To use a course someone sent you, open the `.tqc` file with **Import**;
it is copied into your library.

The library is the `courses` folder of the Torqa data directory:

| System | Folder |
|---|---|
| macOS | `~/Library/Application Support/Torqa/courses` |
| Windows | `%APPDATA%\Torqa\courses` |
| Linux | `~/.local/share/torqa/courses` |

A course's card shows its route on a black-and-white map of the land around the whole route,
with its lakes, rivers, main roads and the names of the towns and villages along it. The map is
downloaded once, when the course is prepared, and kept in the course file as a picture. A
course prepared offline, or by an earlier Torqa with a map of the route's corridor only, gets
the map the next time it is opened with an internet connection.

You can also copy `.tqc` files there directly. A name too long for its card shows in full when
you hover over it. The data directory may live in a synced folder.

## Video courses

Courses ridden along a video refer to the video rather than containing it. Torqa keeps a copy
of the video (a hard link on the same disk) next to the `.tqc` in the library when the course
is prepared; keep the two together when moving or sharing them ([video.md](video.md)).

## Size and attribution

A course takes a few MB; sizes depend mostly on how much terrain and map data the
corridor around the route covers. Every course carries the credits of its data: © OpenFreeMap
© OpenMapTiles, data © OpenStreetMap contributors (ODbL); terrain by Mapterhorn (CC BY 4.0) and
AWS Terrain Tiles. Keep them when you share courses.

Courses saved by a newer Torqa may not open in an older one; update Torqa in that case.
