import { useEffect, useState, type FormEvent } from "react";
import {
  deleteProvider,
  deleteProviderKey,
  listProviders,
  saveProvider,
  selectProvider,
  type Provider,
} from "../lib/daemon";

export type ToolboxSection = "settings" | "about";

type ProviderToolboxProps = {
  section: ToolboxSection;
  onClose: () => void;
};

const EMPTY_PROVIDER = {
  name: "",
  baseUrl: "https://api.openai.com/v1",
  model: "",
  apiKey: "",
};

function ProviderToolbox({ section, onClose }: ProviderToolboxProps) {
  const [providers, setProviders] = useState<Provider[]>([]);
  const [form, setForm] = useState(EMPTY_PROVIDER);
  const [editingId, setEditingId] = useState<string | undefined>();
  const [message, setMessage] = useState("");

  const refresh = async () => {
    try {
      setProviders(await listProviders());
    } catch {
      setMessage("Couldn’t load AI providers.");
    }
  };

  useEffect(() => {
    void refresh();
  }, []);

  const save = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    try {
      await saveProvider({ ...form, id: editingId, apiKey: form.apiKey || undefined });
      setForm(EMPTY_PROVIDER);
      setEditingId(undefined);
      setMessage("Provider saved locally.");
      await refresh();
    } catch (error) {
      setMessage(error instanceof Error ? error.message : "Couldn’t save the provider.");
    }
  };

  const activate = async (provider: Provider) => {
    await selectProvider(provider.id);
    await refresh();
  };

  const remove = async (provider: Provider) => {
    await deleteProvider(provider.id);
    await refresh();
  };

  const clearKey = async (provider: Provider) => {
    await deleteProviderKey(provider.id);
    await refresh();
  };

  const edit = (provider: Provider) => {
    setEditingId(provider.id);
    setForm({
      name: provider.name,
      baseUrl: provider.base_url,
      model: provider.model,
      apiKey: "",
    });
  };

  return (
    <section className="toolbox-card" aria-label="Daemon toolbox">
      <header>
        <span>{section === "about" ? "Daemon" : "Settings"}</span>
        <button type="button" onClick={onClose} aria-label="Close toolbox">×</button>
      </header>
      {section === "about" ? (
        <>
          <p>Daemon v0.1.0 · Local companion · OpenAI-compatible chat endpoints.</p>
          <a href="https://platform.openai.com/docs/api-reference/chat" target="_blank" rel="noreferrer">Chat Completions reference</a>
        </>
      ) : (
        <>
          <p className="toolbox-description">Configure the AI provider, model, and API key here. Keys stay in the OS credential manager.</p>
          <div className="provider-list">
            {providers.map((provider) => (
              <div key={provider.id} className="provider-row">
                <button type="button" className="provider-select" onClick={() => void activate(provider)}>
                  {provider.is_active ? "●" : "○"} {provider.name} · {provider.model}
                </button>
                <span className="provider-actions">
                  <button type="button" onClick={() => edit(provider)}>Edit</button>
                  <button type="button" disabled={!provider.api_key_configured} onClick={() => void clearKey(provider)}>Clear key</button>
                  <button type="button" onClick={() => void remove(provider)}>Remove</button>
                </span>
              </div>
            ))}
          </div>
          <form className="provider-form" onSubmit={save}>
            <input value={form.name} onChange={(event) => setForm({ ...form, name: event.target.value })} placeholder="Provider name" />
            <input value={form.baseUrl} onChange={(event) => setForm({ ...form, baseUrl: event.target.value })} placeholder="Base URL" />
            <input value={form.model} onChange={(event) => setForm({ ...form, model: event.target.value })} placeholder="Model name" />
            <input type="password" value={form.apiKey} onChange={(event) => setForm({ ...form, apiKey: event.target.value })} placeholder="API key" />
            <button type="submit">{editingId ? "Update provider" : "Save provider"}</button>
          </form>
        </>
      )}
      {message && <p className="toolbox-message">{message}</p>}
    </section>
  );
}

export default ProviderToolbox;
