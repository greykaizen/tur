import { ThemeProvider } from "@/components/theme-provider"
// import { SidebarProvider, SidebarTrigger } from "@/components/ui/sidebar"
// import { AppSidebar } from "@/components/app-sidebar"
// import { AppSettingsProvider } from "./providers/AppSettingsProvider";
import { BrowserRouter, Routes, Route } from 'react-router-dom';
import { Home, Settings } from "lucide-react";

export default function App() {
  // tauri store loads here via hook
  return (
    <ThemeProvider defaultTheme="system" storageKey="vite-ui-theme">
      Settings
        {/* <Header /> */}
        <BrowserRouter>
          <Routes>
            <Route path="/" element={<Home />} />
            <Route path="/settings" element={<Settings />} />
          </Routes>
        </BrowserRouter>
    </ThemeProvider>
  )
}