import { lazy, Suspense } from "react";
import { createBrowserRouter, Navigate, RouterProvider } from "react-router";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { LoaderCircle } from "lucide-react";
import { ApiError, setUnauthorizedHandler } from "@/api/client";
import { RequireAuth } from "@/auth";
import { AppLayout } from "@/components/AppLayout";
import { ToastProvider } from "@/components/Toast";
import { PageSpinner } from "@/components/ui";
import { AccountPage } from "@/pages/AccountPage";
import { HomePage } from "@/pages/HomePage";
import { InvitePage } from "@/pages/InvitePage";
import { ItemPage } from "@/pages/item/ItemPage";
import { LibraryPage } from "@/pages/LibraryPage";
import { LoginPage } from "@/pages/LoginPage";
import { NotFoundPage, RouteErrorPage } from "@/pages/NotFoundPage";
import { ProfilesPage } from "@/pages/ProfilesPage";
import { SearchPage } from "@/pages/SearchPage";
import { SetupPage } from "@/pages/SetupPage";

// Route-level code splitting: the player (hls.js) and the admin screens load on demand.
const PlayerPage = lazy(() => import("@/player/PlayerPage").then((m) => ({ default: m.PlayerPage })));
const SettingsLayout = lazy(() => import("@/pages/settings/SettingsLayout").then((m) => ({ default: m.SettingsLayout })));
const LibrariesSettings = lazy(() =>
  import("@/pages/settings/LibrariesSettings").then((m) => ({ default: m.LibrariesSettings })),
);
const UsersSettings = lazy(() => import("@/pages/settings/UsersSettings").then((m) => ({ default: m.UsersSettings })));
const InvitesSettings = lazy(() =>
  import("@/pages/settings/InvitesSettings").then((m) => ({ default: m.InvitesSettings })),
);
const ServerSettings = lazy(() => import("@/pages/settings/ServerSettings").then((m) => ({ default: m.ServerSettings })));
const DashboardSettings = lazy(() =>
  import("@/pages/settings/DashboardSettings").then((m) => ({ default: m.DashboardSettings })),
);

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 30_000,
      gcTime: 10 * 60_000,
      refetchOnWindowFocus: false,
      retry: (count, err) => {
        if (err instanceof ApiError && [400, 401, 403, 404, 409].includes(err.status)) return false;
        return count < 2;
      },
    },
    mutations: { retry: false },
  },
});

function PlayerFallback() {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black">
      <LoaderCircle className="size-12 animate-spin text-white/70" />
    </div>
  );
}

const router = createBrowserRouter([
  { path: "/setup", element: <SetupPage />, errorElement: <RouteErrorPage /> },
  { path: "/login", element: <LoginPage />, errorElement: <RouteErrorPage /> },
  { path: "/invite/:code", element: <InvitePage />, errorElement: <RouteErrorPage /> },
  {
    element: <RequireAuth />,
    errorElement: <RouteErrorPage />,
    children: [
      { path: "/profiles", element: <ProfilesPage /> },
      {
        path: "/play/:id",
        element: (
          <Suspense fallback={<PlayerFallback />}>
            <PlayerPage />
          </Suspense>
        ),
      },
      {
        element: <AppLayout />,
        children: [
          { index: true, element: <HomePage /> },
          { path: "library/:id", element: <LibraryPage /> },
          { path: "item/:id", element: <ItemPage /> },
          { path: "search", element: <SearchPage /> },
          { path: "account", element: <AccountPage /> },
          {
            path: "settings",
            element: (
              <Suspense fallback={<PageSpinner />}>
                <SettingsLayout />
              </Suspense>
            ),
            children: [
              { index: true, element: <Navigate to="libraries" replace /> },
              { path: "libraries", element: <LibrariesSettings /> },
              { path: "users", element: <UsersSettings /> },
              { path: "invitations", element: <InvitesSettings /> },
              { path: "server", element: <ServerSettings /> },
              { path: "dashboard", element: <DashboardSettings /> },
            ],
          },
          { path: "*", element: <NotFoundPage /> },
        ],
      },
    ],
  },
]);

// Any authenticated request that comes back 401 (expired/revoked session) sends the user to the login screen.
setUnauthorizedHandler(() => {
  const { pathname, search } = window.location;
  if (pathname.startsWith("/login") || pathname.startsWith("/setup") || pathname.startsWith("/invite/")) return;
  const next = pathname + search;
  void router
    .navigate(next && next !== "/" ? `/login?next=${encodeURIComponent(next)}` : "/login", { replace: true })
    .then(() => queryClient.clear());
});

export function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <ToastProvider>
        <RouterProvider router={router} />
      </ToastProvider>
    </QueryClientProvider>
  );
}
