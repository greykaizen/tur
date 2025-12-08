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

    const handleKeyUp = (e: KeyboardEvent) => {
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
    set('shortcuts.goHome', 'Ctrl+K');
    set('shortcuts.openSettings', 'Ctrl+P');
    set('shortcuts.addDownload', 'Ctrl+N');
    set('shortcuts.openDetails', 'Ctrl+D');
    set('shortcuts.openHistory', 'Ctrl+H');
    set('shortcuts.toggleSidebar', 'Ctrl+L');
    set('shortcuts.cancelDownload', 'Ctrl+C');
    set('shortcuts.quitApp', 'Ctrl+Q');
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
                      onClick={() => setEditingShortcut('goHome')}
                      className={`px-3 py-1.5 rounded-md transition-colors ${
                        editingShortcut === 'goHome' ? 'bg-primary text-primary-foreground' : 'bg-muted hover:bg-muted/80'
                      }`}
                    >
                      <code className="text-sm font-mono">
                        {editingShortcut === 'goHome' && recordedKeys.length > 0 ? recordedKeys.join('+') : editingShortcut === 'goHome' ? 'Press keys...' : settings.shortcuts.goHome}
                      </code>
                    </button>
                  </div>
                  <div className="flex items-center justify-between">
                    <div className="space-y-0.5">
                      <Label>Open Settings</Label>
                      <p className="text-sm text-muted-foreground">Open settings page</p>
                    </div>
                    <button
                      onClick={() => setEditingShortcut('openSettings')}
                      className={`px-3 py-1.5 rounded-md transition-colors ${
                        editingShortcut === 'openSettings' ? 'bg-primary text-primary-foreground' : 'bg-muted hover:bg-muted/80'
                      }`}
                    >
                      <code className="text-sm font-mono">
                        {editingShortcut === 'openSettings' && recordedKeys.length > 0 ? recordedKeys.join('+') : editingShortcut === 'openSettings' ? 'Press keys...' : settings.shortcuts.openSettings}
                      </code>
                    </button>
                  </div>
                  <div className="flex items-center justify-between">
                    <div className="space-y-0.5">
                      <Label>Add New Download</Label>
                      <p className="text-sm text-muted-foreground">Open add download dialog</p>
                    </div>
                    <button
                      onClick={() => setEditingShortcut('addDownload')}
                      className={`px-3 py-1.5 rounded-md transition-colors ${
                        editingShortcut === 'addDownload' ? 'bg-primary text-primary-foreground' : 'bg-muted hover:bg-muted/80'
                      }`}
                    >
                      <code className="text-sm font-mono">
                        {editingShortcut === 'addDownload' && recordedKeys.length > 0 ? recordedKeys.join('+') : editingShortcut === 'addDownload' ? 'Press keys...' : settings.shortcuts.addDownload}
                      </code>
                    </button>
                  </div>
                  <div className="flex items-center justify-between">
                    <div className="space-y-0.5">
                      <Label>Open Details</Label>
                      <p className="text-sm text-muted-foreground">Navigate to details page</p>
                    </div>
                    <button
                      onClick={() => setEditingShortcut('openDetails')}
                      className={`px-3 py-1.5 rounded-md transition-colors ${
                        editingShortcut === 'openDetails' ? 'bg-primary text-primary-foreground' : 'bg-muted hover:bg-muted/80'
                      }`}
                    >
                      <code className="text-sm font-mono">
                        {editingShortcut === 'openDetails' && recordedKeys.length > 0 ? recordedKeys.join('+') : editingShortcut === 'openDetails' ? 'Press keys...' : settings.shortcuts.openDetails}
                      </code>
                    </button>
                  </div>
                  <div className="flex items-center justify-between">
                    <div className="space-y-0.5">
                      <Label>Open History</Label>
                      <p className="text-sm text-muted-foreground">Navigate to history page</p>
                    </div>
                    <button
                      onClick={() => setEditingShortcut('openHistory')}
                      className={`px-3 py-1.5 rounded-md transition-colors ${
                        editingShortcut === 'openHistory' ? 'bg-primary text-primary-foreground' : 'bg-muted hover:bg-muted/80'
                      }`}
                    >
                      <code className="text-sm font-mono">
                        {editingShortcut === 'openHistory' && recordedKeys.length > 0 ? recordedKeys.join('+') : editingShortcut === 'openHistory' ? 'Press keys...' : settings.shortcuts.openHistory}
                      </code>
                    </button>
                  </div>
                  <div className="flex items-center justify-between">
                    <div className="space-y-0.5">
                      <Label>Toggle Sidebar</Label>
                      <p className="text-sm text-muted-foreground">Open/close sidebar</p>
                    </div>
                    <button
                      onClick={() => setEditingShortcut('toggleSidebar')}
                      className={`px-3 py-1.5 rounded-md transition-colors ${
                        editingShortcut === 'toggleSidebar' ? 'bg-primary text-primary-foreground' : 'bg-muted hover:bg-muted/80'
                      }`}
                    >
                      <code className="text-sm font-mono">
                        {editingShortcut === 'toggleSidebar' && recordedKeys.length > 0 ? recordedKeys.join('+') : editingShortcut === 'toggleSidebar' ? 'Press keys...' : settings.shortcuts.toggleSidebar}
                      </code>
                    </button>
                  </div>
                  <div className="flex items-center justify-between">
                    <div className="space-y-0.5">
                      <Label>Cancel Download</Label>
                      <p className="text-sm text-muted-foreground">Cancel current download</p>
                    </div>
                    <button
                      onClick={() => setEditingShortcut('cancelDownload')}
                      className={`px-3 py-1.5 rounded-md transition-colors ${
                        editingShortcut === 'cancelDownload' ? 'bg-primary text-primary-foreground' : 'bg-muted hover:bg-muted/80'
                      }`}
                    >
                      <code className="text-sm font-mono">
                        {editingShortcut === 'cancelDownload' && recordedKeys.length > 0 ? recordedKeys.join('+') : editingShortcut === 'cancelDownload' ? 'Press keys...' : settings.shortcuts.cancelDownload}
                      </code>
                    </button>
                  </div>
                  <div className="flex items-center justify-between">
                    <div className="space-y-0.5">
                      <Label>Quit Application</Label>
                      <p className="text-sm text-muted-foreground">Quit the application</p>
                    </div>
                    <button
                      onClick={() => setEditingShortcut('quitApp')}
                      className={`px-3 py-1.5 rounded-md transition-colors ${
                        editingShortcut === 'quitApp' ? 'bg-primary text-primary-foreground' : 'bg-muted hover:bg-muted/80'
                      }`}
                    >
                      <code className="text-sm font-mono">
                        {editingShortcut === 'quitApp' && recordedKeys.length > 0 ? recordedKeys.join('+') : editingShortcut === 'quitApp' ? 'Press keys...' : settings.shortcuts.quitApp}
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
                      checked={settings.showNotifications}
                      onCheckedChange={(checked) => set('showNotifications', checked)}
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
                      checked={settings.sendAnonymousMetrics}
                      onCheckedChange={(checked) => set('sendAnonymousMetrics', checked)}
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
