---
sidebar_position: 4
---

# REST API Reference

The Synodic API runs on port 3000 by default. Start it with `synodic serve`.

**Base URL:** `http://localhost:3000/api`

## Endpoints

### Health check

```
GET /api/health
```

**Response:**
```json
{"status": "ok"}
```

---

### List events

```
GET /api/events
```

**Query parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `type` | string | Filter by event type |
| `severity` | string | Filter by severity level |
| `unresolved` | boolean | Only show unresolved events |
| `source` | string | Filter by source |
| `limit` | integer | Max results (default: 100) |

**Response:** `200 OK`
```json
[
  {
    "id": "01HXYZ...",
    "event_type": "tool_call_error",
    "title": "Tool error in bash_123: No such file or directory",
    "severity": "medium",
    "source": "claude",
    "metadata": {"tool_use_id": "bash_123", "error": "No such file or directory"},
    "resolved": false,
    "resolution_notes": null,
    "created_at": "2026-03-24T10:00:00Z",
    "resolved_at": null
  }
]
```

---

### Get event

```
GET /api/events/{id}
```

**Response:** `200 OK` — single event object, or `404 Not Found`.

---

### Submit event

```
POST /api/events
```

**Request body:**
```json
{
  "type": "compliance_violation",
  "title": "API key found in output",
  "severity": "critical",
  "source": "claude",
  "metadata": {"file": "config.py"}
}
```

| Field | Type | Required | Default |
|-------|------|----------|---------|
| `type` | string | yes | — |
| `title` | string | yes | — |
| `severity` | string | no | `"medium"` |
| `source` | string | no | `"api"` |
| `metadata` | object | no | `{}` |

**Response:** `201 Created` — the created event object.

Submitted events are automatically broadcast to WebSocket subscribers.

---

### Resolve event

```
PATCH /api/events/{id}/resolve
```

**Request body:**
```json
{
  "notes": "Fixed in PR #42"
}
```

**Response:** `200 OK`
```json
{"resolved": true, "id": "01HXYZ..."}
```

---

### List rules

```
GET /api/rules
```

**Response:** `200 OK` — array of detection rules.
```json
[
  {
    "name": "secret-in-output",
    "description": "Detects potential secrets or API keys in output",
    "pattern": "(?i)(api[_-]?key|secret|password|token)\\s*[=:]\\s*\\S+",
    "event_type": "compliance_violation",
    "severity": "critical",
    "enabled": true
  }
]
```

---

### Get stats

```
GET /api/stats
```

**Response:** `200 OK`
```json
{
  "total": 42,
  "unresolved": 7,
  "by_type": {
    "tool_call_error": 20,
    "hallucination": 12,
    "compliance_violation": 5,
    "misalignment": 5
  },
  "by_severity": {
    "low": 15,
    "medium": 18,
    "high": 6,
    "critical": 3
  }
}
```
