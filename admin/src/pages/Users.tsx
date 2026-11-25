import { useEffect, useState } from 'react'
import { fetchUsers, logout, updateUser, type User } from '../api'

export default function UsersPage({ user, onLogout }: { user: User; onLogout: () => void }) {
  const [users, setUsers] = useState<User[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState('')
  const [busyId, setBusyId] = useState<string | null>(null)

  async function load() {
    setLoading(true)
    setError('')
    try {
      const res = await fetchUsers()
      setUsers(res.users)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load users')
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    void load()
  }, [])

  async function toggleRole(id: string, field: 'is_staff' | 'is_admin', value: boolean) {
    setBusyId(id)
    setError('')
    try {
      const updated = await updateUser(id, { [field]: value })
      setUsers((prev) => prev.map((u) => (u.id === id ? updated : u)))
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Update failed')
    } finally {
      setBusyId(null)
    }
  }

  async function handleLogout() {
    try {
      await logout()
    } finally {
      onLogout()
    }
  }

  return (
    <div className="page">
      <div className="topbar">
        <div>
          <h1>Users</h1>
          <p className="muted">Signed in as {user.email}</p>
        </div>
        <div className="actions">
          <button className="secondary" onClick={() => void load()} disabled={loading}>Refresh</button>
          <button className="secondary" onClick={() => void handleLogout()}>Logout</button>
        </div>
      </div>

      <div className="card">
        {error && <div className="error">{error}</div>}
        {loading ? (
          <div className="muted">Loading users...</div>
        ) : (
          <div className="table-wrap">
            <table>
              <thead>
                <tr>
                  <th>Name</th>
                  <th>Email</th>
                  <th>Referral</th>
                  <th>Verified</th>
                  <th>Staff</th>
                  <th>Admin</th>
                  <th>Actions</th>
                </tr>
              </thead>
              <tbody>
                {users.map((u) => (
                  <tr key={u.id}>
                    <td>{u.name} {u.surname}</td>
                    <td>{u.email}</td>
                    <td>{u.referral_code || '-'}</td>
                    <td><span className={`badge ${u.is_email_verified ? 'ok' : ''}`}>{u.is_email_verified ? 'Yes' : 'No'}</span></td>
                    <td><span className={`badge ${u.is_staff ? 'ok' : ''}`}>{u.is_staff ? 'Yes' : 'No'}</span></td>
                    <td><span className={`badge ${u.is_admin ? 'ok' : ''}`}>{u.is_admin ? 'Yes' : 'No'}</span></td>
                    <td>
                      <div className="actions">
                        <button className="secondary" disabled={busyId === u.id} onClick={() => void toggleRole(u.id, 'is_staff', !u.is_staff)}>
                          {u.is_staff ? 'Remove staff' : 'Make staff'}
                        </button>
                        <button className="secondary" disabled={busyId === u.id} onClick={() => void toggleRole(u.id, 'is_admin', !u.is_admin)}>
                          {u.is_admin ? 'Remove admin' : 'Make admin'}
                        </button>
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>
    </div>
  )
}
