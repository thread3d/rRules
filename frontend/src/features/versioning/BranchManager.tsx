import { useState } from 'react'

export function BranchManager() {
  const [branches, setBranches] = useState<Branch[]>([])
  const [showCreateModal, setShowCreateModal] = useState(false)

  // Fetch branches on mount
  // useEffect(() => {
  //   fetchBranches()
  // }, [])

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold">Branches</h1>
        <button
          onClick={() => setShowCreateModal(true)}
          className="btn btn-primary"
        >
          New Branch
        </button>
      </div>

      {/* Branch List */}
      <div className="card">
        {branches.length === 0 ? (
          <p className="text-gray-500">No branches found.</p>
        ) : (
          <div className="space-y-2">
            {branches.map((branch) => (
              <div
                key={branch.name}
                className={`flex items-center justify-between p-3 rounded-lg ${
                  branch.isDefault ? 'border-l-4 border-blue-500' : ''
                }`}
              >
                <div>
                  <span className="font-medium">{branch.name}</span>
                  {branch.isDefault && (
                    <span className="badge badge-sm badge-primary ml-2">main</span>
                  )}
                  {branch.lastCommitSha && (
                    <span className="text-xs text-gray-500 ml-2">
                      {branch.lastCommitSha.slice(0, 7)}
                    </span>
                  )}
                </div>

                {!branch.isDefault && (
                  <button
                    onClick={() => handleMerge(branch)}
                    className="btn btn-sm btn-primary"
                  >
                    Merge to main
                  </button>
                )}
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Create Branch Modal */}
      {showCreateModal && (
        <CreateBranchModal onClose={() => setShowCreateModal(false)} />
      )}
    </div>
  )

  async function handleMerge(branch: Branch) {
    if (!window.confirm(`Merge ${branch.name} into main?`)) return

    try {
      // API call to merge branch
      setBranches((prev) => prev.filter((b) => b.name !== branch.name))
    } catch (error) {
      console.error('Merge failed:', error)
    }
  }

  async function fetchBranches() {
    // API call would go here
  }
}

interface Branch {
  id: number
  name: string
  isDefault: boolean
  createdAt: string
  lastCommitSha?: string
}

function CreateBranchModal({ onClose }: { onClose: () => void }) {
  const [name, setName] = useState('')

  return (
    <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center p-4">
      <div className="card w-full max-w-md">
        <h2 className="text-xl font-bold mb-4">Create New Branch</h2>
        <input
          type="text"
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="Branch name (e.g., feature/my-feature)"
          className="input input-bordered w-full mb-4"
        />
        <div className="flex justify-end space-x-2">
          <button onClick={onClose} className="btn btn-secondary">
            Cancel
          </button>
          <button
            onClick={() => {
              // API call to create branch
              onClose()
            }}
            disabled={!name}
            className="btn btn-primary"
          >
            Create Branch
          </button>
        </div>
      </div>
    </div>
  )
}
