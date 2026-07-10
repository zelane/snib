[
  .. | objects | select(.foreign_toplevel_identifier != null)
]
| map({
    key: .foreign_toplevel_identifier,
    value: {
      pid,
      app_id,
      con_id: (.id | tostring)
    }
  })
| from_entries
