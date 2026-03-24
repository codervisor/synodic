---
sidebar_position: 5
---

# WebSocket API

Synodic provides real-time event streaming via WebSocket.

## Connection

```
ws://localhost:3000/api/ws
```

## Message types

### Snapshot (server -> client)

Sent immediately on connection with the 20 most recent events:

```json
{
  "type": "snapshot",
  "events": [
    {
      "id": "01HXYZ...",
      "event_type": "tool_call_error",
      "title": "...",
      "severity": "medium",
      "source": "claude",
      "metadata": {},
      "resolved": false,
      "resolution_notes": null,
      "created_at": "2026-03-24T10:00:00Z",
      "resolved_at": null
    }
  ]
}
```

### Event (server -> client)

Sent whenever a new event is submitted via the API:

```json
{
  "type": "event",
  "event": {
    "id": "01HXYZ...",
    "event_type": "compliance_violation",
    "title": "API key found in output",
    "severity": "critical",
    "source": "claude",
    "metadata": {},
    "resolved": false,
    "resolution_notes": null,
    "created_at": "2026-03-24T10:05:00Z",
    "resolved_at": null
  }
}
```

### Lagged (server -> client)

Sent when the client falls behind the broadcast buffer:

```json
{
  "type": "lagged",
  "missed": 15
}
```

## Example client

### JavaScript

```javascript
const ws = new WebSocket('ws://localhost:3000/api/ws');

ws.onmessage = (msg) => {
  const data = JSON.parse(msg.data);

  switch (data.type) {
    case 'snapshot':
      console.log(`Loaded ${data.events.length} recent events`);
      break;
    case 'event':
      console.log(`New event: [${data.event.severity}] ${data.event.title}`);
      break;
    case 'lagged':
      console.log(`Missed ${data.missed} events`);
      break;
  }
};
```

### curl (websocat)

```bash
websocat ws://localhost:3000/api/ws
```
