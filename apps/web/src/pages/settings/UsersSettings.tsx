import { useState } from "react";
import { EllipsisVertical, KeyRound, Plus, ShieldCheck, ShieldOff, Trash2, UserPlus } from "lucide-react";
import { errorMessage } from "@/api/client";
import { useCreateUser, useDeleteUser, useUpdateUser, useUsers } from "@/api/hooks";
import type { User } from "@/api/types";
import { useAuth } from "@/auth";
import { Menu, MenuItem, MenuSeparator } from "@/components/Menu";
import { ConfirmModal, Modal } from "@/components/Modal";
import { useToast } from "@/components/Toast";
import { Badge, Button, EmptyState, Field, IconButton, PageSpinner, TextInput, Toggle } from "@/components/ui";
import { formatTimestamp } from "@/lib/format";
import { useDocumentTitle } from "@/lib/hooks";

function AddUserModal({ open, onClose }: { open: boolean; onClose: () => void }) {
  const create = useCreateUser();
  const toast = useToast();
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [isAdmin, setIsAdmin] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const reset = () => {
    setUsername("");
    setPassword("");
    setIsAdmin(false);
    setError(null);
  };
  const close = () => {
    reset();
    onClose();
  };

  const submit = () => {
    setError(null);
    if (!username.trim()) return setError("Enter a username.");
    if (password.length < 4) return setError("Use at least 4 characters for the password.");
    create.mutate(
      { username: username.trim(), password, is_admin: isAdmin },
      {
        onSuccess: (u) => {
          toast.success(`Created ${u.username}`);
          close();
        },
        onError: (e) => setError(errorMessage(e)),
      },
    );
  };

  return (
    <Modal
      open={open}
      onClose={close}
      locked={create.isPending}
      title="Add user"
      footer={
        <>
          <Button variant="ghost" onClick={close} disabled={create.isPending}>
            Cancel
          </Button>
          <Button variant="primary" onClick={submit} loading={create.isPending}>
            Create user
          </Button>
        </>
      }
    >
      <form
        className="space-y-4"
        onSubmit={(e) => {
          e.preventDefault();
          submit();
        }}
      >
        <Field label="Username" htmlFor="new-username">
          <TextInput id="new-username" autoComplete="off" value={username} onChange={(e) => setUsername(e.target.value)} />
        </Field>
        <Field label="Password" htmlFor="new-password">
          <TextInput id="new-password" type="password" autoComplete="new-password" value={password} onChange={(e) => setPassword(e.target.value)} />
        </Field>
        <div className="flex items-center justify-between rounded-md border border-border bg-surface-2 px-3 py-2.5">
          <div>
            <div className="text-sm font-medium">Administrator</div>
            <div className="text-xs text-subtle">Can manage libraries, users and server settings.</div>
          </div>
          <Toggle checked={isAdmin} onChange={setIsAdmin} label="Administrator" />
        </div>
        {error ? <p className="text-sm text-danger">{error}</p> : null}
        <button type="submit" className="hidden" />
      </form>
    </Modal>
  );
}

function ResetPasswordModal({ user, onClose }: { user: User | null; onClose: () => void }) {
  const update = useUpdateUser();
  const toast = useToast();
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);

  const submit = () => {
    if (!user) return;
    setError(null);
    if (password.length < 4) return setError("Use at least 4 characters.");
    update.mutate(
      { id: user.id, password },
      {
        onSuccess: () => {
          toast.success(`Password reset for ${user.username}`);
          setPassword("");
          onClose();
        },
        onError: (e) => setError(errorMessage(e)),
      },
    );
  };

  return (
    <Modal
      open={!!user}
      onClose={() => {
        setPassword("");
        setError(null);
        onClose();
      }}
      locked={update.isPending}
      size="sm"
      title={`Reset password for ${user?.username ?? ""}`}
      footer={
        <>
          <Button variant="ghost" onClick={onClose} disabled={update.isPending}>
            Cancel
          </Button>
          <Button variant="primary" onClick={submit} loading={update.isPending}>
            Reset password
          </Button>
        </>
      }
    >
      <form
        onSubmit={(e) => {
          e.preventDefault();
          submit();
        }}
        className="space-y-3"
      >
        <Field label="New password" htmlFor="reset-password">
          <TextInput id="reset-password" type="password" autoComplete="new-password" value={password} onChange={(e) => setPassword(e.target.value)} />
        </Field>
        {error ? <p className="text-sm text-danger">{error}</p> : null}
      </form>
    </Modal>
  );
}

export function UsersSettings() {
  useDocumentTitle("Users");
  const me = useAuth();
  const users = useUsers();
  const update = useUpdateUser();
  const del = useDeleteUser();
  const toast = useToast();
  const [adding, setAdding] = useState(false);
  const [resetting, setResetting] = useState<User | null>(null);
  const [deleting, setDeleting] = useState<User | null>(null);

  if (users.isPending) return <PageSpinner />;
  if (users.isError) return <EmptyState title="Couldn't load users" description={errorMessage(users.error)} />;

  const adminCount = users.data.filter((u) => u.is_admin).length;

  return (
    <div className="space-y-5">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h2 className="text-lg font-semibold">Users</h2>
          <p className="text-sm text-muted">Everyone with an account can browse and play all libraries.</p>
        </div>
        <Button variant="primary" icon={<Plus className="size-4" />} onClick={() => setAdding(true)}>
          Add user
        </Button>
      </div>

      {users.data.length === 0 ? (
        <EmptyState icon={<UserPlus />} title="No users" />
      ) : (
        <div className="overflow-hidden rounded-lg border border-border">
          <table className="w-full text-left text-sm">
            <thead className="bg-surface-2 text-xs uppercase tracking-wider text-subtle">
              <tr>
                <th className="px-4 py-2.5 font-semibold">User</th>
                <th className="px-4 py-2.5 font-semibold">Role</th>
                <th className="hidden px-4 py-2.5 font-semibold sm:table-cell">Created</th>
                <th className="w-12 px-2 py-2.5" />
              </tr>
            </thead>
            <tbody className="divide-y divide-white/[0.05] bg-surface">
              {users.data.map((u) => {
                const self = u.id === me.id;
                const lastAdmin = u.is_admin && adminCount <= 1;
                return (
                  <tr key={u.id}>
                    <td className="px-4 py-3">
                      <div className="flex items-center gap-3">
                        <span className="flex size-8 items-center justify-center rounded-full bg-surface-3 text-xs font-bold uppercase">
                          {u.username.slice(0, 1)}
                        </span>
                        <span className="font-medium">{u.username}</span>
                        {self ? <span className="text-xs text-subtle">(you)</span> : null}
                      </div>
                    </td>
                    <td className="px-4 py-3">{u.is_admin ? <Badge tone="accent">Admin</Badge> : <Badge>User</Badge>}</td>
                    <td className="hidden px-4 py-3 text-muted sm:table-cell">{formatTimestamp(u.created_at)}</td>
                    <td className="px-2 py-3 text-right">
                      <Menu
                        trigger={({ toggle }) => (
                          <IconButton label={`Actions for ${u.username}`} size="sm" onClick={toggle}>
                            <EllipsisVertical className="size-4" />
                          </IconButton>
                        )}
                      >
                        {(close) => (
                          <>
                            <MenuItem
                              icon={u.is_admin ? <ShieldOff /> : <ShieldCheck />}
                              disabled={self || lastAdmin}
                              onClick={() => {
                                close();
                                update.mutate(
                                  { id: u.id, is_admin: !u.is_admin },
                                  {
                                    onSuccess: () => toast.success(u.is_admin ? `${u.username} is no longer an admin` : `${u.username} is now an admin`),
                                    onError: (e) => toast.error(errorMessage(e)),
                                  },
                                );
                              }}
                            >
                              {u.is_admin ? "Remove admin" : "Make admin"}
                            </MenuItem>
                            <MenuItem
                              icon={<KeyRound />}
                              onClick={() => {
                                close();
                                setResetting(u);
                              }}
                            >
                              Reset password
                            </MenuItem>
                            <MenuSeparator />
                            <MenuItem
                              icon={<Trash2 />}
                              danger
                              disabled={self}
                              onClick={() => {
                                close();
                                setDeleting(u);
                              }}
                            >
                              Delete user
                            </MenuItem>
                          </>
                        )}
                      </Menu>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}

      <AddUserModal open={adding} onClose={() => setAdding(false)} />
      <ResetPasswordModal user={resetting} onClose={() => setResetting(null)} />
      <ConfirmModal
        open={!!deleting}
        onClose={() => setDeleting(null)}
        title="Delete user?"
        danger
        confirmLabel="Delete"
        loading={del.isPending}
        message={
          <>
            <strong className="text-fg">{deleting?.username}</strong> will lose access and their watch history will be deleted.
          </>
        }
        onConfirm={() => {
          if (!deleting) return;
          del.mutate(deleting.id, {
            onSuccess: () => {
              toast.success(`Deleted ${deleting.username}`);
              setDeleting(null);
            },
            onError: (e) => toast.error(errorMessage(e)),
          });
        }}
      />
    </div>
  );
}
