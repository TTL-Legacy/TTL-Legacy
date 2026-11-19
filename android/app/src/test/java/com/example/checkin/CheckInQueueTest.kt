package com.example.checkin

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

/**
 * Unit tests for [CheckInQueue] covering enqueue/dequeue ordering,
 * persistence across restarts, and duplicate suppression.
 */
class CheckInQueueTest {

    private lateinit var storage: InMemoryCheckInStorage
    private lateinit var queue: CheckInQueue

    @Before
    fun setUp() {
        storage = InMemoryCheckInStorage()
        queue = CheckInQueue(storage)
    }

    @Test
    fun enqueue_thenDequeue_returnsItemsInFifoOrder() {
        queue.enqueue(CheckIn(id = "a", timestamp = 1L))
        queue.enqueue(CheckIn(id = "b", timestamp = 2L))
        queue.enqueue(CheckIn(id = "c", timestamp = 3L))

        assertEquals("a", queue.dequeue()?.id)
        assertEquals("b", queue.dequeue()?.id)
        assertEquals("c", queue.dequeue()?.id)
    }

    @Test
    fun dequeue_onEmptyQueue_returnsNull() {
        assertEquals(null, queue.dequeue())
    }

    @Test
    fun size_reflectsEnqueuedAndDequeuedItems() {
        assertEquals(0, queue.size())
        queue.enqueue(CheckIn(id = "a", timestamp = 1L))
        queue.enqueue(CheckIn(id = "b", timestamp = 2L))
        assertEquals(2, queue.size())
        queue.dequeue()
        assertEquals(1, queue.size())
    }

    @Test
    fun enqueue_persistsAcrossRestart() {
        queue.enqueue(CheckIn(id = "a", timestamp = 1L))
        queue.enqueue(CheckIn(id = "b", timestamp = 2L))

        // Simulate a process restart by rebuilding the queue over the same storage.
        val restarted = CheckInQueue(storage)

        assertEquals(2, restarted.size())
        assertEquals("a", restarted.dequeue()?.id)
        assertEquals("b", restarted.dequeue()?.id)
    }

    @Test
    fun dequeue_persistsRemovalAcrossRestart() {
        queue.enqueue(CheckIn(id = "a", timestamp = 1L))
        queue.enqueue(CheckIn(id = "b", timestamp = 2L))
        queue.dequeue()

        val restarted = CheckInQueue(storage)

        assertEquals(1, restarted.size())
        assertEquals("b", restarted.dequeue()?.id)
    }

    @Test
    fun enqueue_duplicateId_isSuppressed() {
        assertTrue(queue.enqueue(CheckIn(id = "a", timestamp = 1L)))
        assertFalse(queue.enqueue(CheckIn(id = "a", timestamp = 2L)))

        assertEquals(1, queue.size())
        assertEquals("a", queue.dequeue()?.id)
    }

    @Test
    fun enqueue_duplicateId_doesNotOverwriteOriginal() {
        queue.enqueue(CheckIn(id = "a", timestamp = 1L))
        queue.enqueue(CheckIn(id = "a", timestamp = 99L))

        assertEquals(1L, queue.dequeue()?.timestamp)
    }

    @Test
    fun enqueue_duplicateSuppression_persistsAcrossRestart() {
        queue.enqueue(CheckIn(id = "a", timestamp = 1L))

        val restarted = CheckInQueue(storage)
        assertFalse(restarted.enqueue(CheckIn(id = "a", timestamp = 2L)))
        assertEquals(1, restarted.size())
    }

    @Test
    fun contains_reflectsQueuedItems() {
        queue.enqueue(CheckIn(id = "a", timestamp = 1L))

        assertTrue(queue.contains("a"))
        assertFalse(queue.contains("missing"))
    }
}
