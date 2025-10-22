import { useSettings } from '@/hooks/useSettings';

export default function SettingsPage() {
  const { ready, settings, set } = useSettings();

  if (!ready) return null;

  return (
    <>
      <label>Theme:
        <select
          value={settings.app.theme}
          onChange={e => set('app.theme', e.target.value)}
        >
          <option value="light">Light</option>
          <option value="dark">Dark</option>
          <option value="system">System</option>
        </select>
      </label>
    </>
  );
}
