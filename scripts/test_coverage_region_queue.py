#!/usr/bin/env python3
"""Regression checks for queue bookkeeping, using an isolated synthetic report."""
import contextlib
import io
import json
from pathlib import Path
from tempfile import TemporaryDirectory
from types import SimpleNamespace
import unittest

import build_coverage_region_queue as queue


class QueueTests(unittest.TestCase):
    def setUp(self):
        self.temp = TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.db = self.root / 'queue.duckdb'
        self.report = self.root / 'coverage.json'
        self.write_report(0, 0)
        with contextlib.redirect_stdout(io.StringIO()):
            queue.seed(SimpleNamespace(coverage_json=self.report, db=self.db,
                snapshot_id='seed', baseline_snapshot_id='base', batch_id='batch', batch_size=2))
        self.ids = [row[0] for row in self.query('SELECT region_id FROM region_queue ORDER BY start_column')]

    def query(self, sql):
        with queue.duckdb_module().connect(str(self.db)) as conn:
            return conn.execute(sql).fetchall()

    def write_report(self, first, second):
        self.report.write_text(json.dumps({'data': [{'files': [{'filename': 'src/example.rs'}],
            'functions': [{'name': 'example', 'filenames': ['src/example.rs'],
                'regions': [[10, 1, 10, 3, first], [10, 5, 10, 8, second]]}]}]}))

    def packet(self, case_id, region_ids):
        return {'case_id': case_id, 'target': 'src/example.rs:10', 'target_region_ids': region_ids}

    def import_cases(self, *cases):
        path = self.root / 'packets.json'
        path.write_text(json.dumps({'cases': list(cases)}))
        with contextlib.redirect_stdout(io.StringIO()):
            queue.import_packets(SimpleNamespace(db=self.db, packets=path, batch_id='batch', agent_prefix='review'))

    def reconcile(self, kind='incremental', selected=None, status='passed'):
        with contextlib.redirect_stdout(io.StringIO()):
            queue.reconcile(SimpleNamespace(db=self.db, coverage_json=self.report,
                snapshot_id='observed', run_id='run', batch_id='batch',
                verification_kind=kind, run_status=status, case_id=selected))

    def test_explicit_ids_do_not_expand_to_other_regions_on_same_line(self):
        self.import_cases(self.packet('one', self.ids[:1]))
        self.assertEqual(self.query('SELECT region_id FROM case_region_plan'), [(self.ids[0],)])

    def test_repaired_mapping_replaces_links_but_preserves_history(self):
        self.import_cases(self.packet('one', self.ids[:1]))
        self.import_cases(self.packet('one', self.ids[1:]))
        self.assertEqual(self.query('SELECT region_id FROM case_region_plan'), [(self.ids[1],)])
        self.assertEqual(self.query("SELECT count(*) FROM queue_history WHERE event='case_planned'"), [(2,)])

    def test_invalid_packet_rolls_back_whole_import(self):
        with self.assertRaises(SystemExit):
            self.import_cases(self.packet('one', self.ids[:1]), self.packet('bad', ['unknown']))
        self.assertEqual(self.query('SELECT count(*) FROM case_plan'), [(0,)])

    def test_slice_does_not_count_unexecuted_cases_as_misses(self):
        self.import_cases(self.packet('one', self.ids[:1]), self.packet('two', self.ids[1:]))
        self.write_report(1, 0)
        self.reconcile(selected=['one'])
        self.assertEqual(self.query('SELECT status,tries FROM region_queue ORDER BY start_column'),
            [('hit_pending_full', 0), ('pending', 0)])
        self.assertEqual(self.query("SELECT count(*) FROM queue_history WHERE run_id='run'"), [(1,)])

    def test_incremental_miss_preserves_hit_until_full_verification(self):
        self.import_cases(self.packet('one', self.ids[:1]))
        self.write_report(1, 0)
        self.reconcile()
        self.write_report(0, 0)
        self.reconcile()
        self.assertEqual(self.query('SELECT status,tries FROM region_queue ORDER BY start_column')[0], ('hit_pending_full', 1))
        self.reconcile('complete')
        self.assertEqual(self.query('SELECT status,tries FROM region_queue ORDER BY start_column')[0], ('pending', 2))

    def test_complete_hit_survives_failed_incremental_attempt(self):
        self.import_cases(self.packet('one', self.ids[:1]))
        self.write_report(1, 0)
        self.reconcile('complete')
        self.reconcile(status='failed')
        self.assertEqual(self.query('SELECT status,tries FROM region_queue ORDER BY start_column')[0], ('done', 1))
        self.assertEqual(self.query("SELECT count(*) FROM queue_history WHERE event='failed'"), [(1,)])

    def test_unknown_slice_and_filtered_complete_rejected(self):
        self.import_cases(self.packet('one', self.ids[:1]))
        for kind, ids in [('incremental', ['unknown']), ('complete', ['one'])]:
            with self.assertRaises(SystemExit):
                self.reconcile(kind, ids)
        self.assertEqual(self.query("SELECT count(*) FROM queue_history WHERE run_id='run'"), [(0,)])

    def test_shared_target_records_every_case_without_duplicate_tries(self):
        self.import_cases(self.packet('one', self.ids[:1]), self.packet('two', self.ids[:1]))
        self.reconcile(selected=['one', 'two'])
        self.assertEqual(self.query('SELECT tries FROM region_queue ORDER BY start_column'), [(1,), (0,)])
        self.assertEqual(self.query("SELECT case_id,try_no FROM queue_history WHERE run_id='run' ORDER BY case_id"),
            [('one', 1), ('two', 1)])


if __name__ == '__main__':
    unittest.main()
