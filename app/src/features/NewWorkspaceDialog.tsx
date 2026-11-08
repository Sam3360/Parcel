import { useEffect, useState } from 'react';
import { Dialog } from '../components/Dialog';
import { Button, ErrorText, Field } from '../components/ui';
import { describeError } from '../lib/api';
import { useApp } from '../hooks/app';
import type { Template, Workspace } from '../lib/types';

export function NewWorkspaceDialog({
  initialName = '',
  initialTemplateId = '',
  onClose,
  onCreated,
}: {
  initialName?: string;
  initialTemplateId?: string;
  onClose: () => void;
  onCreated: (w: Workspace) => void;
}) {
  const { api } = useApp();
  const [name, setName] = useState(initialName);
  const [description, setDescription] = useState('');
  const [templateId, setTemplateId] = useState(initialTemplateId);
  const [templates, setTemplates] = useState<Template[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    api
      .listTemplates()
      .then(setTemplates)
      .catch(() => {});
  }, [api]);
  const chosen = templates.find((t) => t.id === templateId);
  const submit = async () => {
    setBusy(true);
    setError(null);
    try {
      onCreated(await api.createWorkspace(name.trim(), description, templateId || null));
    } catch (e) {
      setError(describeError(e));
      setBusy(false);
    }
  };
  return (
    <Dialog
      title="New workspace"
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="primary" disabled={!name.trim() || busy} onClick={submit}>
            Create workspace
          </Button>
        </>
      }
    >
      <form
        className="stack"
        onSubmit={(e) => {
          e.preventDefault();
          if (name.trim() && !busy) void submit();
        }}
      >
        <Field label="Name" htmlFor="ws-name">
          <input
            id="ws-name"
            className="input"
            value={name}
            onChange={(e) => setName(e.target.value)}
            maxLength={200}
          />
        </Field>
        <Field label="Description (optional)" htmlFor="ws-desc">
          <textarea
            id="ws-desc"
            className="textarea"
            value={description}
            onChange={(e) => setDescription(e.target.value)}
          />
        </Field>
        <Field
          label="Start from"
          htmlFor="ws-tpl"
          hint={
            chosen
              ? `${chosen.description} Creates ${chosen.spec.folders.length} folders and ${chosen.spec.notes.length} starter notes.`
              : 'An empty workspace.'
          }
        >
          <select
            id="ws-tpl"
            className="select"
            value={templateId}
            onChange={(e) => setTemplateId(e.target.value)}
          >
            <option value="">Blank workspace</option>
            {templates.map((t) => (
              <option key={t.id} value={t.id}>
                {t.name}
              </option>
            ))}
          </select>
        </Field>
        <ErrorText error={error} />
      </form>
    </Dialog>
  );
}
