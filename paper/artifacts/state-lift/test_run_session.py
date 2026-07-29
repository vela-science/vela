#!/usr/bin/env python3
"""Tests for state-lift session output capture."""

from __future__ import annotations

import json
import unittest

from run_session import (
    SessionError,
    extract_answer_from_events,
    extract_usage_from_events,
)


class ExtractAnswerTests(unittest.TestCase):
    def test_extracts_last_agent_message(self) -> None:
        first = {
            "type": "item.completed",
            "item": {"type": "agent_message", "text": '{"attempt":1}'},
        }
        final = {
            "type": "item.completed",
            "item": {"type": "agent_message", "text": '{"answer":2}'},
        }
        events = (
            json.dumps({"type": "thread.started"})
            + "\n"
            + json.dumps(first)
            + "\n"
            + json.dumps(final)
            + "\n"
        ).encode()

        answer, line_number = extract_answer_from_events(events)

        self.assertEqual(answer, b'{"answer":2}\n')
        self.assertEqual(line_number, 3)

    def test_rejects_non_json_final_message(self) -> None:
        events = json.dumps(
            {
                "type": "item.completed",
                "item": {"type": "agent_message", "text": "not json"},
            }
        ).encode()

        with self.assertRaisesRegex(SessionError, "not a JSON object"):
            extract_answer_from_events(events)

    def test_rejects_malformed_event_stream(self) -> None:
        with self.assertRaisesRegex(SessionError, "invalid JSON"):
            extract_answer_from_events(b"{")


class ExtractUsageTests(unittest.TestCase):
    def test_extracts_terminal_usage_and_observed_total(self) -> None:
        events = (
            json.dumps(
                {
                    "type": "turn.completed",
                    "usage": {
                        "input_tokens": 40,
                        "cached_input_tokens": 30,
                        "output_tokens": 2,
                        "reasoning_output_tokens": 1,
                    },
                }
            )
            + "\n"
        ).encode()

        usage = extract_usage_from_events(events)

        self.assertEqual(usage["observed_tokens"], 42)
        self.assertEqual(usage["cached_input_tokens"], 30)

    def test_requires_completed_turn(self) -> None:
        with self.assertRaisesRegex(SessionError, "no completed turn"):
            extract_usage_from_events(
                (json.dumps({"type": "thread.started"}) + "\n").encode()
            )


if __name__ == "__main__":
    unittest.main()
