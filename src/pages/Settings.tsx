import { useState, useEffect } from 'react';
import PageTransition from '@/components/PageTransition';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { Label } from '@/components/ui/label';
import { Switch } from '@/components/ui/switch';
import { Button } from '@/components/ui/button';
import { useSettings } from '@/contexts/SettingsContext';

export default function Settings() {
  const { settings, set, ready } = useSettings();
  const [activeTab, setActiveTab] = useState('general');
  const [editingShortcut, setEditingShortcut] = useState<string | null>(null);
  const [recordedKeys, setRecordedKeys] = useState<string[]>([]);

  useEffect(() => {
    if (!editingShortcut) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();

      const keys: string[] = [];
      if (e.ctrlKey) keys.push('Ctrl');
      if (e.shiftKey) keys.push('Shift');
      if (e.altKey) keys.push('Alt');
      if (e.metaKey) keys.push('Meta');

      const key = e.key.toUpperCase();
      if (!['CONTROL', 'SHIFT', 'ALT', 'META'].includes(key)) {
        keys.push(key);
      }

      if (keys.length > 0 && !['CONTROL', 'SHIFT', 'ALT', 'META'].includes(key)) {
        setRecordedKeys(keys);
      }
    };

    const handleKeyUp = (_e: KeyboardEvent) => {
      if (recordedKeys.length > 0) {
        const shortcut = recordedKeys.join('+');
        set(`shortcuts.${editingShortcut}`, shortcut);
        setEditingShortcut(null);
        setRecordedKeys([]);
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    window.addEventListener('keyup', handleKeyUp);

    return () => {
      window.removeEventListener('keydown', handleKeyDown);
      window.removeEventListener('keyup', handleKeyUp);
    };
  }, [editingShortcut, recordedKeys, set]);

  const resetShortcuts = () => {
    set('shortcuts.go_home', 'Ctrl+K');
    set('shortcuts.open_settings', 'Ctrl+P');
    set('shortcuts.add_download', 'Ctrl+N');
    set('shortcuts.open_details', 'Ctrl+D');
    set('shortcuts.open_history', 'Ctrl+H');
    set('shortcuts.toggle_sidebar', 'Ctrl+L');
    set('shortcuts.cancel_download', 'Ctrl+C');
    set('shortcuts.quit_app', 'Ctrl+Q');
  };

  if (!ready) {
    return (
      <PageTransition>
        <div className="flex items-center justify-center h-full">
          <p className="text-muted-foreground">Loading settings...</p>
        </div>
      </PageTransition>
    );
  }

  return (
    <PageTransition>
      <div className="h-full w-full overflow-y-auto p-6">
        <div className="max-w-3xl mx-auto space-y-6">
          {/* <h1 className="text-2xl font-bold">Settings</h1> */}
          
          <Tabs value={activeTab} onValueChange={setActiveTab} className="w-full relative z-0">
            <div className="w-full overflow-x-auto">
              <TabsList className="inline-flex w-full min-w-max">
                <TabsTrigger value="general" className="flex-1 min-w-0">General</TabsTrigger>
                <TabsTrigger value="appearance" className="flex-1 min-w-0">Appearance</TabsTrigger>
                <TabsTrigger value="shortcuts" className="flex-1 min-w-0">Shortcuts</TabsTrigger>
                <TabsTrigger value="notifications" className="flex-1 min-w-0">Notifications</TabsTrigger>
                <TabsTrigger value="app" className="flex-1 min-w-0">App</TabsTrigger>
              </TabsList>
            </div>

            <TabsContent value="general" className="space-y-6 mt-6">
              <div className="space-y-4">
                <div className="space-y-4">
                  <div className="flex items-center justify-between">
                    <div className="space-y-0.5">
                      <Label>Number of Threads</Label>
                      <p className="text-sm text-muted-foreground">
                        Concurrent download threads (Current: {settings.download.num_threads})
                      </p>
                    </div>
                  </div>
                  <div className="flex items-center justify-between">
                    <div className="space-y-0.5">
                      <Label>Save History</Label>
                      <p className="text-sm text-muted-foreground">
                        Keep download history
                      </p>
                    </div>
                    <Switch
                      checked={settings.session.history}
                      onCheckedChange={(checked) => set('session.history', checked)}
                    />
                  </div>
                  <div className="flex items-center justify-between">
                    <div className="space-y-0.5">
                      <Label>Save Metadata</Label>
                      <p className="text-sm text-muted-foreground">
                        Store download metadata
                      </p>
                    </div>
                    <Switch
                      checked={settings.session.metadata}
                      onCheckedChange={(checked) => set('session.metadata', checked)}
                    />
                  </div>
                </div>
              </div>
            </TabsContent>

            <TabsContent value="appearance" className="space-y-6 mt-6">
              <div className="space-y-4">
                <div className="space-y-4">
                  <div className="flex items-center justify-between">
                    <div className="space-y-0.5">
                      <Label>Sidebar Position</Label>
                      <p className="text-sm text-muted-foreground">
                        Choose where the sidebar appears
                      </p>
                    </div>
                    <div className="flex gap-2">
                      <button
                        onClick={() => set('app.sidebar', 'left')}
                        className={`px-4 py-2 rounded-md text-sm font-medium transition-colors ${
                          settings.app.sidebar === 'left'
                            ? 'bg-primary text-primary-foreground'
                            : 'bg-muted hover:bg-muted/80'
                        }`}
                      >
                        Left
                      </button>
                      <button
                        onClick={() => set('app.sidebar', 'right')}
                        className={`px-4 py-2 rounded-md text-sm font-medium transition-colors ${
                          settings.app.sidebar === 'right'
                            ? 'bg-primary text-primary-foreground'
                            : 'bg-muted hover:bg-muted/80'
                        }`}
                      >
                        Right
                      </button>
                    </div>
                  </div>
                  <div className="flex items-center justify-between">
                    <div className="space-y-0.5">
                      <Label>Button Labels</Label>
                      <p className="text-sm text-muted-foreground">
                        Display text, icon, or both on buttons
                      </p>
                    </div>
                    <div className="flex gap-2">
                      <button
                        onClick={() => set('app.button_label', 'text')}
                        className={`px-4 py-2 rounded-md text-sm font-medium transition-colors ${
                          settings.app.button_label === 'text'
                            ? 'bg-primary text-primary-foreground'
                            : 'bg-muted hover:bg-muted/80'
                        }`}
                      >
                        Text
                      </button>
                      <button
                        onClick={() => set('app.button_label', 'icon')}
                        className={`px-4 py-2 rounded-md text-sm font-medium transition-colors ${
                          settings.app.button_label === 'icon'
                            ? 'bg-primary text-primary-foreground'
                            : 'bg-muted hover:bg-muted/80'
                        }`}
                      >
                        Icon
                      </button>
                      <button
                        onClick={() => set('app.button_label', 'both')}
                        className={`px-4 py-2 rounded-md text-sm font-medium transition-colors ${
                          settings.app.button_label === 'both'
                            ? 'bg-primary text-primary-foreground'
                            : 'bg-muted hover:bg-muted/80'
                        }`}
                      >
                        Both
                      </button>
                    </div>
                  </div>
                  <div className="flex items-center justify-between">
                    <div className="space-y-0.5">
                      <Label>Show Download Progress Bar</Label>
                      <p className="text-sm text-muted-foreground">
                        Display main download progress bar
                      </p>
                    </div>
                    <Switch
                      checked={settings.app.show_download_progress}
                      onCheckedChange={(checked) => set('app.show_download_progress', checked)}
                    />
                  </div>
                  <div className="flex items-center justify-between">
                    <div className="space-y-0.5">
                      <Label>Show Connection Segment Bar</Label>
                      <p className="text-sm text-muted-foreground">
                        Display connection segment visualization
                      </p>
                    </div>
                    <Switch
                      checked={settings.app.show_segment_progress}
                      onCheckedChange={(checked) => set('app.show_segment_progress', checked)}
                    />
                  </div>
                </div>
              </div>
            </TabsContent>

            <TabsContent value="shortcuts" className="space-y-6 mt-6">
              <div className="flex justify-end mb-4">
                <Button variant="outline" size="sm" onClick={resetShortcuts}>
                  Reset to Defaults
                </Button>
              </div>
              <div className="space-y-4">
                <div className="space-y-4">
                  <div className="flex items-center justify-between">
                    <div className="space-y-0.5">
                      <Label>Go to Home</Label>
                      <p className="text-sm text-muted-foreground">Navigate to home page</p>
                    </div>
                    <button
                      onClick={() => setEditingShortcut('go_home')}
                      className={`px-3 py-1.5 rounded-md transition-colors ${
                        editingShortcut === 'go_home' ? 'bg-primary text-primary-foreground' : 'bg-muted hover:bg-muted/80'
                      }`}
                    >
                      <code className="text-sm font-mono">
                        {editingShortcut === 'go_home' && recordedKeys.length > 0 ? recordedKeys.join('+') : editingShortcut === 'go_home' ? 'Press keys...' : settings.shortcuts.go_home}
                      </code>
                    </button>
                  </div>
                  <div className="flex items-center justify-between">
                    <div className="space-y-0.5">
                      <Label>Open Settings</Label>
                      <p className="text-sm text-muted-foreground">Open settings page</p>
                    </div>
                    <button
                      onClick={() => setEditingShortcut('open_settings')}
                      className={`px-3 py-1.5 rounded-md transition-colors ${
                        editingShortcut === 'open_settings' ? 'bg-primary text-primary-foreground' : 'bg-muted hover:bg-muted/80'
                      }`}
                    >
                      <code className="text-sm font-mono">
                        {editingShortcut === 'open_settings' && recordedKeys.length > 0 ? recordedKeys.join('+') : editingShortcut === 'open_settings' ? 'Press keys...' : settings.shortcuts.open_settings}
                      </code>
                    </button>
                  </div>
                  <div className="flex items-center justify-between">
                    <div className="space-y-0.5">
                      <Label>Add New Download</Label>
                      <p className="text-sm text-muted-foreground">Open add download dialog</p>
                    </div>
                    <button
                      onClick={() => setEditingShortcut('add_download')}
                      className={`px-3 py-1.5 rounded-md transition-colors ${
                        editingShortcut === 'add_download' ? 'bg-primary text-primary-foreground' : 'bg-muted hover:bg-muted/80'
                      }`}
                    >
                      <code className="text-sm font-mono">
                        {editingShortcut === 'add_download' && recordedKeys.length > 0 ? recordedKeys.join('+') : editingShortcut === 'add_download' ? 'Press keys...' : settings.shortcuts.add_download}
                      </code>
                    </button>
                  </div>
                  <div className="flex items-center justify-between">
                    <div className="space-y-0.5">
                      <Label>Open Details</Label>
                      <p className="text-sm text-muted-foreground">Navigate to details page</p>
                    </div>
                    <button
                      onClick={() => setEditingShortcut('open_details')}
                      className={`px-3 py-1.5 rounded-md transition-colors ${
                        editingShortcut === 'open_details' ? 'bg-primary text-primary-foreground' : 'bg-muted hover:bg-muted/80'
                      }`}
                    >
                      <code className="text-sm font-mono">
                        {editingShortcut === 'open_details' && recordedKeys.length > 0 ? recordedKeys.join('+') : editingShortcut === 'open_details' ? 'Press keys...' : settings.shortcuts.open_details}
                      </code>
                    </button>
                  </div>
                  <div className="flex items-center justify-between">
                    <div className="space-y-0.5">
                      <Label>Open History</Label>
                      <p className="text-sm text-muted-foreground">Navigate to history page</p>
                    </div>
                    <button
                      onClick={() => setEditingShortcut('open_history')}
                      className={`px-3 py-1.5 rounded-md transition-colors ${
                        editingShortcut === 'open_history' ? 'bg-primary text-primary-foreground' : 'bg-muted hover:bg-muted/80'
                      }`}
                    >
                      <code className="text-sm font-mono">
                        {editingShortcut === 'open_history' && recordedKeys.length > 0 ? recordedKeys.join('+') : editingShortcut === 'open_history' ? 'Press keys...' : settings.shortcuts.open_history}
                      </code>
                    </button>
                  </div>
                  <div className="flex items-center justify-between">
                    <div className="space-y-0.5">
                      <Label>Toggle Sidebar</Label>
                      <p className="text-sm text-muted-foreground">Open/close sidebar</p>
                    </div>
                    <button
                      onClick={() => setEditingShortcut('toggle_sidebar')}
                      className={`px-3 py-1.5 rounded-md transition-colors ${
                        editingShortcut === 'toggle_sidebar' ? 'bg-primary text-primary-foreground' : 'bg-muted hover:bg-muted/80'
                      }`}
                    >
                      <code className="text-sm font-mono">
                        {editingShortcut === 'toggle_sidebar' && recordedKeys.length > 0 ? recordedKeys.join('+') : editingShortcut === 'toggle_sidebar' ? 'Press keys...' : settings.shortcuts.toggle_sidebar}
                      </code>
                    </button>
                  </div>
                  <div className="flex items-center justify-between">
                    <div className="space-y-0.5">
                      <Label>Cancel Download</Label>
                      <p className="text-sm text-muted-foreground">Cancel current download</p>
                    </div>
                    <button
                      onClick={() => setEditingShortcut('cancel_download')}
                      className={`px-3 py-1.5 rounded-md transition-colors ${
                        editingShortcut === 'cancel_download' ? 'bg-primary text-primary-foreground' : 'bg-muted hover:bg-muted/80'
                      }`}
                    >
                      <code className="text-sm font-mono">
                        {editingShortcut === 'cancel_download' && recordedKeys.length > 0 ? recordedKeys.join('+') : editingShortcut === 'cancel_download' ? 'Press keys...' : settings.shortcuts.cancel_download}
                      </code>
                    </button>
                  </div>
                  <div className="flex items-center justify-between">
                    <div className="space-y-0.5">
                      <Label>Quit Application</Label>
                      <p className="text-sm text-muted-foreground">Quit the application</p>
                    </div>
                    <button
                      onClick={() => setEditingShortcut('quit_app')}
                      className={`px-3 py-1.5 rounded-md transition-colors ${
                        editingShortcut === 'quit_app' ? 'bg-primary text-primary-foreground' : 'bg-muted hover:bg-muted/80'
                      }`}
                    >
                      <code className="text-sm font-mono">
                        {editingShortcut === 'quit_app' && recordedKeys.length > 0 ? recordedKeys.join('+') : editingShortcut === 'quit_app' ? 'Press keys...' : settings.shortcuts.quit_app}
                      </code>
                    </button>
                  </div>
                </div>
              </div>
            </TabsContent>

            <TabsContent value="notifications" className="space-y-6 mt-6">
              <div className="space-y-4">
                <div className="space-y-4">
                  <div className="flex items-center justify-between">
                    <div className="space-y-0.5">
                      <Label>Show Notifications</Label>
                      <p className="text-sm text-muted-foreground">
                        Display notifications for download events
                      </p>
                    </div>
                    <Switch
                      checked={settings.show_notifications}
                      onCheckedChange={(checked) => set('show_notifications', checked)}
                    />
                  </div>
                </div>
              </div>
            </TabsContent>

            <TabsContent value="app" className="space-y-6 mt-6">
              <div className="space-y-4">
                <div className="space-y-4">
                  <div className="flex items-center justify-between">
                    <div className="space-y-0.5">
                      <Label>Show Tray Icon</Label>
                      <p className="text-sm text-muted-foreground">
                        Display app icon in system tray
                      </p>
                    </div>
                    <Switch
                      checked={settings.app.show_tray_icon}
                      onCheckedChange={(checked) => set('app.show_tray_icon', checked)}
                    />
                  </div>
                  <div className="flex items-center justify-between">
                    <div className="space-y-0.5">
                      <Label>Quit on Close</Label>
                      <p className="text-sm text-muted-foreground">
                        Exit app when window is closed
                      </p>
                    </div>
                    <Switch
                      checked={settings.app.quit_on_close}
                      onCheckedChange={(checked) => set('app.quit_on_close', checked)}
                    />
                  </div>
                  <div className="flex items-center justify-between">
                    <div className="space-y-0.5">
                      <Label>Send Anonymous Metrics</Label>
                      <p className="text-sm text-muted-foreground">
                        Help improve the app by sharing usage data
                      </p>
                    </div>
                    <Switch
                      checked={settings.send_anonymous_metrics}
                      onCheckedChange={(checked) => set('send_anonymous_metrics', checked)}
                    />
                  </div>
                </div>
              </div>
            </TabsContent>
          </Tabs>
        </div>
      </div>
    </PageTransition>
  );
}
