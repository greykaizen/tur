import { useSettings } from '@/hooks/useSettings';
import { useState } from 'react';

export default function Header() {
  const { submitWork } = useSettings();
  const [value, setValue] = useState('');

  return (
    <header>
      <input
        placeholder="New work..."
        value={value}
        onChange={e => setValue(e.target.value)}
      />
      <button onClick={() => submitWork(value)}>New</button>
    </header>
  );
}

// Frontend
// invoke("handle_instance", {
//   target: { type: "urls", data: ["https://..."] }
// });

// invoke("handle_instance", {
//   target: { type: "uuids", data: ["550e8400-e29b-41d4-a716-446655440000"] }
// });

