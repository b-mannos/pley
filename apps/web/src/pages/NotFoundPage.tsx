import { Link, isRouteErrorResponse, useRouteError } from "react-router";
import { Compass, TriangleAlert } from "lucide-react";
import { Button, buttonClass, EmptyState } from "@/components/ui";
import { useDocumentTitle } from "@/lib/hooks";

export function NotFoundPage() {
  useDocumentTitle("Not found");
  return (
    <EmptyState
      className="min-h-[70vh]"
      icon={<Compass />}
      title="Page not found"
      description="The page you're looking for doesn't exist or was moved."
      action={
        <Link to="/" className={buttonClass("primary")}>
          Go home
        </Link>
      }
    />
  );
}

/** Router-level error boundary. */
export function RouteErrorPage() {
  const error = useRouteError();
  const message = isRouteErrorResponse(error)
    ? `${error.status} ${error.statusText}`
    : error instanceof Error
      ? error.message
      : "Unexpected error";
  return (
    <div className="flex min-h-screen items-center justify-center">
      <EmptyState
        icon={<TriangleAlert />}
        title="Something went wrong"
        description={message}
        action={
          <div className="flex gap-2">
            <Button onClick={() => window.location.reload()}>Reload</Button>
            <a href="/" className={buttonClass("primary")}>
              Go home
            </a>
          </div>
        }
      />
    </div>
  );
}
