import { useCallback, useEffect, useState } from 'react';
import { useApp } from '../hooks/app';
import { Badge, Button, EmptyState, ErrorText, Field, Loading } from '../components/ui';
import { Dialog } from '../components/Dialog';
import { NewWorkspaceDialog } from '../features/NewWorkspaceDialog';
import { describeError } from '../lib/api';
import { plural } from '../lib/format';
import type { ExportProfile, ProfileSpec, Template, WorkspaceSummary } from '../lib/types';

export function TemplatesPage() {
  const { navigate } = useApp();
  const [tab, setTab] = useState<'templates' | 'profiles'>('templates');
  const [useTpl, setUseTpl] = useState<string | null>(null);
  return (
    <div className="page">
      <div className="page-narrow">
        <div className="page-head">
          <div>
            <h1>Templates</h1>
            <p>Reusable workspace layouts and packaging preferences.</p>
          </div>
        </div>
        <div className="tabs" role="tablist">
          <button
            role="tab"
            className="tab"
            aria-selected={tab === 'templates'}
            onClick={() => setTab('templates')}
          >
            Workspace templates
          </button>
          <button
            role="tab"
            className="tab"
            aria-selected={tab === 'profiles'}
            onClick={() => setTab('profiles')}
          >
            Export profiles
          </button>
        </div>
        <div style={{ paddingTop: 18 }}>
          {tab === 'templates' ? <TemplateList onUse={setUseTpl} /> : <ProfileList />}
        </div>
        {useTpl && (
          <NewWorkspaceDialog
            initialTemplateId={useTpl}
            onClose={() => setUseTpl(null)}
            onCreated={(w) => {
              setUseTpl(null);
              navigate({ page: 'workspaces', workspaceId: w.id });
            }}
          />
        )}
      </div>
    </div>
  );
}

function TemplateList({ onUse }: { onUse: (id: string) => void }) {
  const { api, confirm, toast, reportError } = useApp();
  const [items, setItems] = useState<Template[] | null>(null);
  const [edit, setEdit] = useState<Template | 'new' | null>(null);
  const [fromWs, setFromWs] = useState(false);
  const load = useCallback(() => {
    api
      .listTemplates()
      .then(setItems)
      .catch((e) => {
        setItems([]);
        reportError(e);
      });
  }, [api, reportError]);
  useEffect(load, [load]);
  const del = async (t: Template) => {
    if (
      !(await confirm({
        title: 'Delete template?',
        confirmLabel: 'Delete',
        danger: true,
        message: <p>“{t.name}” will be deleted. Workspaces already created from it are not affected.</p>,
      }))
    )
      return;
    try {
      await api.deleteTemplate(t.id);
      load();
    } catch (e) {
      reportError(e);
    }
  };
  if (!items) return <Loading />;
  return (
    <>
      <div className="row" style={{ marginBottom: 12 }}>
        <Button variant="primary" icon="plus" onClick={() => setEdit('new')}>
          New template
        </Button>
        <Button icon="copy" onClick={() => setFromWs(true)}>
          Save a workspace as template…
        </Button>
      </div>
      <div className="grid-cards">
        {items.map((t) => (
          <div key={t.id} className="card card-pad stack" style={{ gap: 6 }}>
            <div className="row">
              <strong className="grow">{t.name}</strong>
              {t.builtin && <Badge>Built-in</Badge>}
            </div>
            <p className="muted small" style={{ minHeight: 34 }}>
              {t.description}
            </p>
            <p className="faint small">
              {plural(t.spec.folders.length, 'folder')} · {plural(t.spec.notes.length, 'starter note')} ·{' '}
              {plural(t.spec.metadata_fields.length, 'field')}
            </p>
            <div className="row">
              <Button variant="primary" onClick={() => onUse(t.id)}>
                Use template
              </Button>
              <Button variant="ghost" onClick={() => setEdit(t)}>
                {t.builtin ? 'Save a copy…' : 'Edit'}
              </Button>
              {!t.builtin && (
                <Button variant="ghost" className="danger" onClick={() => del(t)}>
                  Delete
                </Button>
              )}
            </div>
          </div>
        ))}
      </div>
      {edit && (
        <TemplateEditor
          template={edit === 'new' ? null : edit}
          onClose={() => setEdit(null)}
          onSaved={() => {
            setEdit(null);
            load();
            toast('Template saved', 'success');
          }}
        />
      )}
      {fromWs && (
        <FromWorkspace
          onClose={() => setFromWs(false)}
          onSaved={() => {
            setFromWs(false);
            load();
            toast('Template created from workspace', 'success');
          }}
        />
      )}
    </>
  );
}

function TemplateEditor({
  template,
  onClose,
  onSaved,
}: {
  template: Template | null;
  onClose: () => void;
  onSaved: () => void;
}) {
  const { api } = useApp();
  const copy = template?.builtin;
  const [name, setName] = useState(template ? (copy ? `${template.name} (copy)` : template.name) : '');
  const [desc, setDesc] = useState(template?.description ?? '');
  const [folders, setFolders] = useState((template?.spec.folders ?? []).join('\n'));
  const [fields, setFields] = useState(
    (template?.spec.metadata_fields ?? []).map((f) => `${f.key}=${f.default}`).join('\n'),
  );
  const [error, setError] = useState<string | null>(null);
  const save = async () => {
    try {
      const spec = {
        folders: folders
          .split('\n')
          .map((s) => s.trim().replace(/^\/+|\/+$/g, ''))
          .filter(Boolean),
        notes: template?.spec.notes ?? [],
        metadata_fields: fields
          .split('\n')
          .map((l) => l.trim())
          .filter(Boolean)
          .map((l) => {
            const [k, ...r] = l.split('=');
            return { key: k.trim(), label: k.trim(), default: r.join('=').trim() };
          }),
        default_profile: template?.spec.default_profile ?? null,
      };
      await api.saveTemplate(copy ? null : (template?.id ?? null), name, desc, spec);
      onSaved();
    } catch (e) {
      setError(describeError(e));
    }
  };
  return (
    <Dialog
      title={template && !copy ? 'Edit template' : 'New template'}
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="primary" disabled={!name.trim()} onClick={save}>
            Save template
          </Button>
        </>
      }
    >
      <div className="stack">
        <Field label="Name" htmlFor="t-name">
          <input id="t-name" className="input" value={name} onChange={(e) => setName(e.target.value)} />
        </Field>
        <Field label="Description" htmlFor="t-desc">
          <input id="t-desc" className="input" value={desc} onChange={(e) => setDesc(e.target.value)} />
        </Field>
        <Field
          label="Folders"
          htmlFor="t-folders"
          hint="One per line. Use / for nested folders, e.g. Sources/Papers."
        >
          <textarea
            id="t-folders"
            className="textarea"
            rows={6}
            value={folders}
            onChange={(e) => setFolders(e.target.value)}
          />
        </Field>
        <Field
          label="Metadata fields"
          htmlFor="t-fields"
          hint="One per line as key=default value. Shown on the workspace."
        >
          <textarea
            id="t-fields"
            className="textarea"
            rows={3}
            value={fields}
            onChange={(e) => setFields(e.target.value)}
          />
        </Field>
        {template && template.spec.notes.length > 0 && (
          <p className="muted small">{plural(template.spec.notes.length, 'starter note')} will be kept.</p>
        )}
        <ErrorText error={error} />
      </div>
    </Dialog>
  );
}

function FromWorkspace({ onClose, onSaved }: { onClose: () => void; onSaved: () => void }) {
  const { api } = useApp();
  const [list, setList] = useState<WorkspaceSummary[]>([]);
  const [ws, setWs] = useState('');
  const [name, setName] = useState('');
  const [notes, setNotes] = useState(true);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    api.listWorkspaces(false).then((l) => {
      setList(l);
      if (l[0]) setWs(l[0].id);
    });
  }, [api]);
  const save = async () => {
    try {
      await api.templateFromWorkspace(ws, name, '', notes);
      onSaved();
    } catch (e) {
      setError(describeError(e));
    }
  };
  return (
    <Dialog
      title="Save workspace as template"
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="primary" disabled={!ws || !name.trim()} onClick={save}>
            Create template
          </Button>
        </>
      }
    >
      {list.length === 0 ? (
        <EmptyState icon="layers" title="No workspaces">
          Create a workspace first.
        </EmptyState>
      ) : (
        <div className="stack">
          <p className="muted">
            Captures the folder structure, metadata fields and (optionally) notes. Files are never included.
          </p>
          <Field label="Workspace" htmlFor="tw-ws">
            <select id="tw-ws" className="select" value={ws} onChange={(e) => setWs(e.target.value)}>
              {list.map((w) => (
                <option key={w.id} value={w.id}>
                  {w.name}
                </option>
              ))}
            </select>
          </Field>
          <Field label="Template name" htmlFor="tw-name">
            <input id="tw-name" className="input" value={name} onChange={(e) => setName(e.target.value)} />
          </Field>
          <label className="check">
            <input type="checkbox" checked={notes} onChange={(e) => setNotes(e.target.checked)} /> Include
            notes as starter notes
          </label>
          <ErrorText error={error} />
        </div>
      )}
    </Dialog>
  );
}

function ProfileList() {
  const { api, confirm, toast, reportError } = useApp();
  const [items, setItems] = useState<ExportProfile[] | null>(null);
  const [edit, setEdit] = useState<ExportProfile | 'new' | null>(null);
  const load = useCallback(() => {
    api
      .listProfiles()
      .then(setItems)
      .catch((e) => {
        setItems([]);
        reportError(e);
      });
  }, [api, reportError]);
  useEffect(load, [load]);
  if (!items) return <Loading />;
  const del = async (p: ExportProfile) => {
    if (
      !(await confirm({
        title: 'Delete export profile?',
        confirmLabel: 'Delete',
        danger: true,
        message: <p>Workspaces using “{p.name}” will fall back to the Standard profile.</p>,
      }))
    )
      return;
    try {
      await api.deleteProfile(p.id);
      load();
    } catch (e) {
      reportError(e);
    }
  };
  return (
    <>
      <p className="muted" style={{ marginBottom: 12 }}>
        Export profiles control how a workspace is packaged. Choose one per workspace in its export dialog.
      </p>
      <Button variant="primary" icon="plus" onClick={() => setEdit('new')}>
        New profile
      </Button>
      <div className="list" style={{ marginTop: 12 }}>
        {items.map((p) => (
          <div key={p.id} className="list-row">
            <div className="grow">
              <strong>{p.name}</strong> {p.builtin && <Badge>Built-in</Badge>}
              <div className="muted small">
                {p.description || (p.spec.compress ? 'Compressed' : 'Uncompressed')}
              </div>
            </div>
            <span className="muted small">
              {p.spec.compress ? 'compressed' : 'stored'}
              {p.spec.exclude_patterns.length ? ` · ${p.spec.exclude_patterns.length} exclusions` : ''}
            </span>
            <Button variant="ghost" onClick={() => setEdit(p)}>
              {p.builtin ? 'Save a copy…' : 'Edit'}
            </Button>
            {!p.builtin && (
              <Button variant="ghost" className="danger" onClick={() => del(p)}>
                Delete
              </Button>
            )}
          </div>
        ))}
      </div>
      {edit && (
        <ProfileEditor
          profile={edit === 'new' ? null : edit}
          onClose={() => setEdit(null)}
          onSaved={() => {
            setEdit(null);
            load();
            toast('Profile saved', 'success');
          }}
        />
      )}
    </>
  );
}

function ProfileEditor({
  profile,
  onClose,
  onSaved,
}: {
  profile: ExportProfile | null;
  onClose: () => void;
  onSaved: () => void;
}) {
  const { api } = useApp();
  const copy = profile?.builtin;
  const [name, setName] = useState(profile ? (copy ? `${profile.name} (copy)` : profile.name) : '');
  const [desc, setDesc] = useState(profile?.description ?? '');
  const [spec, setSpec] = useState<ProfileSpec>(
    profile?.spec ?? {
      compress: true,
      verify_after_write: true,
      include_workspace_notes: true,
      include_references: true,
      exclude_patterns: [],
      filename_pattern: '{name}',
    },
  );
  const [patterns, setPatterns] = useState((profile?.spec.exclude_patterns ?? []).join('\n'));
  const [error, setError] = useState<string | null>(null);
  const chk = (
    k: 'compress' | 'verify_after_write' | 'include_workspace_notes' | 'include_references',
    label: string,
  ) => (
    <label className="check">
      <input
        type="checkbox"
        checked={spec[k]}
        onChange={(e) => setSpec({ ...spec, [k]: e.target.checked })}
      />{' '}
      {label}
    </label>
  );
  const save = async () => {
    try {
      await api.saveProfile(copy ? null : (profile?.id ?? null), name, desc, {
        ...spec,
        exclude_patterns: patterns
          .split('\n')
          .map((s) => s.trim())
          .filter(Boolean),
      });
      onSaved();
    } catch (e) {
      setError(describeError(e));
    }
  };
  return (
    <Dialog
      title={profile && !copy ? 'Edit export profile' : 'New export profile'}
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="primary" disabled={!name.trim()} onClick={save}>
            Save profile
          </Button>
        </>
      }
    >
      <div className="stack">
        <Field label="Name" htmlFor="p-name">
          <input id="p-name" className="input" value={name} onChange={(e) => setName(e.target.value)} />
        </Field>
        <Field label="Description" htmlFor="p-desc">
          <input id="p-desc" className="input" value={desc} onChange={(e) => setDesc(e.target.value)} />
        </Field>
        {chk('compress', 'Compress files')}
        {chk('verify_after_write', 'Verify the package after writing')}
        {chk('include_workspace_notes', 'Include workspace notes')}
        {chk('include_references', 'Include references')}
        <Field
          label="Exclude files matching"
          htmlFor="p-ex"
          hint="One pattern per line; * and ? wildcards, e.g. *.tmp"
        >
          <textarea
            id="p-ex"
            className="textarea"
            rows={3}
            value={patterns}
            onChange={(e) => setPatterns(e.target.value)}
          />
        </Field>
        <Field
          label="Suggested file name"
          htmlFor="p-fn"
          hint="{name} is replaced by the workspace name, {date} by today’s date."
        >
          <input
            id="p-fn"
            className="input"
            value={spec.filename_pattern}
            onChange={(e) => setSpec({ ...spec, filename_pattern: e.target.value })}
          />
        </Field>
        <ErrorText error={error} />
      </div>
    </Dialog>
  );
}
