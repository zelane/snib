[
  .. | objects | select(.stableId != null)
]
| map({
    key: .stableId,
    value: {
      pid,
      address
    }
  })
| from_entries
