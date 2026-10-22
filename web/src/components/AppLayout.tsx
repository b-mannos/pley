import { Suspense, useCallback, useState } from "react";
import { Outlet, ScrollRestoration } from "react-router";
import { AudioPlayerBar } from "@/audio/AudioPlayerBar";
import { useAudioPlayer } from "@/audio/AudioPlayer";
import { cn } from "@/lib/cn";
import { Sidebar } from "./Sidebar";
import { TopBar } from "./TopBar";
import { PageSpinner } from "./ui";

export function AppLayout() {
  const [drawer, setDrawer] = useState(false);
  const closeDrawer = useCallback(() => setDrawer(false), []);
  const audio = useAudioPlayer();

  return (
    <div className="min-h-screen">
      <Sidebar open={drawer} onClose={closeDrawer} />
      <div className="flex min-h-screen flex-col lg:pl-60">
        <TopBar onMenu={() => setDrawer(true)} />
        <main className={cn("flex-1", audio.current && "pb-24")}>
          <Suspense fallback={<PageSpinner />}>
            <Outlet />
          </Suspense>
        </main>
      </div>
      {audio.current ? <AudioPlayerBar /> : null}
      <ScrollRestoration />
    </div>
  );
}
