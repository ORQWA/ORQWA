"""Tests for compare.py: the before/after table a pull request shows."""

import pathlib
import sys
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import compare

HEADER = (
    "label,quality,place_m,sample,fps,median_ms,p95_ms,"
    "view_draw_calls,view_primitives,shadow_draw_calls,shadow_primitives"
)


def rows(*lines):
    return list(compare.csv.DictReader([HEADER, *lines]))


class TableTest(unittest.TestCase):
    def test_timed_runs_compare_frame_times_in_the_order_they_ran(self):
        table = compare.table(
            rows(
                "before,ultra,2510,all,9.8,102.40,106.50,2303,10550795,3124,12631852",
                "after,ultra,2510,all,51.3,19.50,19.80,2303,10550795,3124,12631852",
            )
        )
        self.assertIn("Median frame time", table)
        self.assertIn("| Place (m) | Sample | before | after | Change |", table)
        self.assertIn("| 2510 | all | 102.4 ms (10 fps) | 19.5 ms (51 fps) | -81% |", table)

    def test_software_runs_compare_what_was_drawn(self):
        table = compare.table(
            rows(
                "far,ultra,2903,all,0.0,0.00,0.00,3966,17849191,1997,11845959",
                "near,ultra,2903,all,0.0,0.00,0.00,3000,8000000,1500,6000000",
            )
        )
        self.assertIn("Draw calls / primitives, view + shadows", table)
        self.assertIn("3,966 / 17,849,191 + 1,997 / 11,845,959", table)
        self.assertIn("-53% |", table)

    def test_labels_pick_and_order_the_columns_and_gaps_show_as_dashes(self):
        table = compare.table(
            rows(
                "a,ultra,100,all,50.0,20.00,21.00,1,1,1,1",
                "b,ultra,100,all,60.0,16.00,17.00,1,1,1,1",
                "b,ultra,100,-msaa,70.0,14.00,15.00,1,1,1,1",
            ),
            ["b", "a"],
        )
        self.assertIn("| b | a | Change |", table)
        self.assertIn("| 100 | all | 16.0 ms (60 fps) | 20.0 ms (50 fps) | +25% |", table)
        self.assertIn("| 100 | -msaa | 14.0 ms (70 fps) | – | – |", table)

    def test_rows_of_runs_not_compared_are_left_out(self):
        table = compare.table(
            rows(
                "a,ultra,100,all,50.0,20.00,21.00,1,1,1,1",
                "b,ultra,100,all,60.0,16.00,17.00,1,1,1,1",
                "c,ultra,200,all,40.0,25.00,26.00,1,1,1,1",
            ),
            ["a", "b"],
        )
        self.assertNotIn("| 200 |", table)

    def test_a_place_visited_again_is_a_row_of_its_own(self):
        table = compare.table(
            rows(
                "a,ultra,2510,all,55.0,18.20,18.90,1,1,1,1",
                "a,ultra,2510,all,50.0,20.10,20.60,1,1,1,1",
                "b,ultra,2510,all,61.5,16.25,16.50,1,1,1,1",
                "b,ultra,2510,all,61.5,16.26,16.50,1,1,1,1",
            )
        )
        self.assertIn("| 2510 | all | 18.2 ms (55 fps) | 16.2 ms (62 fps) | -11% |", table)
        self.assertIn(
            "| 2510 (visit 2) | all | 20.1 ms (50 fps) | 16.3 ms (62 fps) | -19% |", table
        )


if __name__ == "__main__":
    unittest.main()
