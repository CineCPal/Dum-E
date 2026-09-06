import { Bot, Info, Settings } from "lucide-react";
import { useState } from "react";
import { AboutDialog } from "@/components/about/AboutDialog";
import { ChatView } from "@/components/chat/ChatView";
import { SettingsDialog } from "@/components/settings/SettingsDialog";
import { Button } from "@/components/ui/button";

function App() {
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [aboutOpen, setAboutOpen] = useState(false);

  return (
    <div className="flex h-screen flex-col bg-background text-foreground">
      <header className="flex items-center justify-between border-b border-border px-5 py-3">
        <div className="flex items-center gap-2">
          <div className="flex size-7 items-center justify-center rounded-full bg-primary text-primary-foreground">
            <Bot className="size-4" />
          </div>
          <span className="font-semibold tracking-tight">Dum-E</span>
          <span className="hidden text-xs text-muted-foreground sm:inline">
            your video production know-it-all
          </span>
        </div>
        <div className="flex items-center gap-1">
          <Button variant="ghost" size="icon" onClick={() => setAboutOpen(true)} aria-label="About">
            <Info className="size-4" />
          </Button>
          <Button variant="ghost" size="icon" onClick={() => setSettingsOpen(true)} aria-label="Settings">
            <Settings className="size-4" />
          </Button>
        </div>
      </header>

      <main className="min-h-0 flex-1">
        <ChatView />
      </main>

      <SettingsDialog open={settingsOpen} onOpenChange={setSettingsOpen} />
      <AboutDialog open={aboutOpen} onOpenChange={setAboutOpen} />
    </div>
  );
}

export default App;
