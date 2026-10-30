"""In-memory micro-batch buffer for high-throughput live market streams."""
import threading
import time
from typing import Callable, List
from src.core.types import AggregatorCandle


class MicroBatchBuffer:
    """Thread-safe micro-batch accumulator for incoming market updates."""

    def __init__(
        self,
        flush_callback: Callable[[List[AggregatorCandle]], None],
        flush_seconds: float = 5.0,
        max_rows: int = 10_000,
    ):
        self._callback = flush_callback
        self._flush_seconds = flush_seconds
        self._max_rows = max_rows
        self._buffer: List[AggregatorCandle] = []
        self._lock = threading.Lock()
        self._last_flush = time.monotonic()

    def add(self, item: AggregatorCandle) -> bool:
        """Adds an item to the buffer. Flushes immediately if batch limit is met."""
        should_flush = False
        with self._lock:
            self._buffer.append(item)
            now = time.monotonic()
            if (
                len(self._buffer) >= self._max_rows
                or (now - self._last_flush) >= self._flush_seconds
            ):
                should_flush = True

        if should_flush:
            self.flush()
        return should_flush

    def flush(self) -> int:
        """Flushes buffered records to the configured writer."""
        to_flush: List[AggregatorCandle] = []
        with self._lock:
            if not self._buffer:
                return 0
            to_flush = self._buffer
            self._buffer = []
            self._last_flush = time.monotonic()

        self._callback(to_flush)
        return len(to_flush)

    @property
    def pending_count(self) -> int:
        with self._lock:
            return len(self._buffer)
