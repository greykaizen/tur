import { useState } from 'react';
import DownloadItem from '@/components/DownloadItem';
import PageTransition from '@/components/PageTransition';

export default function Home() {
  // Mock data - will be replaced with real data later
  const [downloads] = useState([
    {
      id: '1',
      url: 'https://excalidraw.com/',
      size: '150 MB',
      downloaded: '78 MB',
      speed: '2.5 MB/s',
      timeLeft: '28s',
      resume: true,
      progress: 52
    }
  ]);

  if (downloads.length === 0) {
    return (
      <PageTransition>
        <div className="flex items-start justify-center h-full pt-20">
          <div className="text-center text-muted-foreground max-w-md space-y-1">
            <p>Start a New Download by</p>
            <p>clicking on Add button</p>
            <p>or drag N drop a file here</p>
          </div>
        </div>
      </PageTransition>
    );
  }

  return (
    <PageTransition className="p-3 space-y-2 overflow-y-auto">
      {downloads.map((download) => (
        <DownloadItem key={download.id} {...download} />
      ))}
    </PageTransition>
  );
}
