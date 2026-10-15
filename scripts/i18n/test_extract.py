"""Gettext contexts and escaping for the individually reviewed German catalogue."""
import pathlib
import tempfile
import unittest

import extract


class CatalogueTests(unittest.TestCase):
    def test_same_source_can_have_distinct_contexts(self):
        with tempfile.TemporaryDirectory() as directory:
            po = pathlib.Path(directory) / "de.po"
            po.write_text(
                'msgctxt "ride_action"\nmsgid "Ride"\nmsgstr "Fahren"\n\n'
                'msgctxt "ride_tab"\nmsgid "Ride"\nmsgstr "Fahrt"\n'
            )
            self.assertEqual(extract.translated(po), {
                ("ride_action", "Ride"): "Fahren", ("ride_tab", "Ride"): "Fahrt"
            })
            template = extract.template({("ride_action", "Ride"): ["button:1"]})
            self.assertIn('msgctxt "ride_action"\nmsgid "Ride"', template)

    def test_quotes_backslashes_and_unicode_round_trip(self):
        original = 'Das Fahrerprofil "Müller" in C:\\Räder\\%s'
        self.assertEqual(extract.unescape(extract.quote(original)[1:-1]), original)

if __name__ == "__main__":
    unittest.main()
