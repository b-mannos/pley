import { Navigate, useNavigate, useParams, useSearchParams } from "react-router";
import { LoaderCircle, TriangleAlert } from "lucide-react";
import { errorMessage } from "@/api/client";
import { useItem } from "@/api/hooks";
import { VideoPlayer } from "./VideoPlayer";

function num(v: string | null): number | null {
  if (v == null || v === "") return null;
  const n = Number(v);
  return Number.isFinite(n) ? n : null;
}

function PlayerMessage({ title, message, onBack }: { title: string; message: string; onBack: () => void }) {
  return (
    <div className="fixed inset-0 z-50 flex flex-col items-center justify-center gap-4 bg-black p-6 text-center text-white">
      <TriangleAlert className="size-12 text-accent" />
      <div>
        <div className="text-lg font-semibold">{title}</div>
        <p className="mt-1 text-sm text-white/70">{message}</p>
      </div>
      <button type="button" onClick={onBack} className="rounded-md bg-white/10 px-5 py-2 text-sm font-semibold hover:bg-white/20">
        Go back
      </button>
    </div>
  );
}

export function PlayerPage() {
  const { id } = useParams();
  const [params] = useSearchParams();
  const navigate = useNavigate();
  const itemId = Number(id);
  const item = useItem(Number.isFinite(itemId) ? itemId : null);

  if (!Number.isFinite(itemId) || itemId <= 0) return <Navigate to="/" replace />;
  if (item.isPending) {
    return (
      <div className="fixed inset-0 z-50 flex items-center justify-center bg-black">
        <LoaderCircle className="size-12 animate-spin text-white/70" />
      </div>
    );
  }
  if (item.isError) {
    return <PlayerMessage title="Can't play this item" message={errorMessage(item.error)} onBack={() => navigate(-1)} />;
  }

  const data = item.data;
  if (data.kind !== "movie" && data.kind !== "episode") return <Navigate to={`/item/${data.id}`} replace />;
  if (!data.files.length) {
    return <PlayerMessage title="Nothing to play" message="This item has no media files." onBack={() => navigate(-1)} />;
  }

  return (
    <VideoPlayer
      key={data.id}
      item={data}
      startMs={num(params.get("t")) ?? 0}
      fileId={num(params.get("file"))}
      audio={num(params.get("audio"))}
      sub={num(params.get("sub"))}
    />
  );
}
