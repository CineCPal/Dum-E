import { CheckCircle2, Loader2, XCircle } from "lucide-react";
import { useEffect } from "react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Separator } from "@/components/ui/separator";
import { useAboutInfo } from "@/hooks/useAboutInfo";
import { cn } from "@/lib/utils";
import type { ModelStatus } from "@/lib/types";

function StatusRow({ label, ok, detail }: { label: string; ok: boolean; detail?: string }) {
  return (
    <div className="flex items-center justify-between text-sm">
      <span className="text-muted-foreground">{label}</span>
      <span className={cn("flex items-center gap-1.5 font-medium", ok ? "text-emerald-500" : "text-destructive")}>
        {ok ? <CheckCircle2 className="size-4" /> : <XCircle className="size-4" />}
        {detail}
      </span>
    </div>
  );
}

function ModelRow({ label, model }: { label: string; model: ModelStatus }) {
  const detail = !model.available ? "not pulled" : model.loaded_in_memory ? "loaded" : "available";
  return <StatusRow label={label} ok={model.available} detail={`${model.name} (${detail})`} />;
}

export function AboutDialog({ open, onOpenChange }: { open: boolean; onOpenChange: (open: boolean) => void }) {
  const { info, loading, refresh } = useAboutInfo();

  useEffect(() => {
    if (open) refresh();
  }, [open, refresh]);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>About Dum-E</DialogTitle>
          <DialogDescription>
            {info ? `Version ${info.app_version}` : "Your local video production know-it-all."}
          </DialogDescription>
        </DialogHeader>

        {loading && !info ? (
          <div className="flex items-center justify-center py-8 text-muted-foreground">
            <Loader2 className="size-5 animate-spin" />
          </div>
        ) : info ? (
          <div className="flex flex-col gap-4">
            <div className="flex flex-col gap-2">
              <StatusRow label="Ollama" ok={info.ollama_reachable} detail={info.ollama_reachable ? "reachable" : "not reachable"} />
              <ModelRow label="Chat model" model={info.chat_model} />
              <ModelRow label="Embedding model" model={info.embedding_model} />
            </div>

            <Separator />

            <div className="flex flex-col gap-2">
              <div className="flex items-center justify-between text-sm">
                <span className="text-muted-foreground">Manuals indexed</span>
                <span className="font-medium">{info.index_stats.manual_count}</span>
              </div>
              <div className="flex items-center justify-between text-sm">
                <span className="text-muted-foreground">Chunks indexed</span>
                <span className="font-medium">{info.index_stats.chunk_count}</span>
              </div>
              <div className="flex items-center justify-between text-sm">
                <span className="text-muted-foreground">Last indexed</span>
                <span className="font-medium">
                  {info.index_stats.last_indexed_at
                    ? new Date(info.index_stats.last_indexed_at).toLocaleString()
                    : "never"}
                </span>
              </div>
            </div>

            <Separator />

            <StatusRow
              label="DaVinci Resolve"
              ok={info.resolve_reachable}
              detail={info.resolve_reachable ? "reachable" : "not running"}
            />

            <StatusRow
              label="Premiere Pro"
              ok={info.premiere_reachable}
              detail={info.premiere_reachable ? "reachable" : "not connected"}
            />

            <Separator />

            <StatusRow
              label="Internet fallback"
              ok={info.fallback_enabled && info.searxng_reachable}
              detail={
                !info.searxng_reachable
                  ? "SearXNG not reachable"
                  : info.fallback_enabled
                    ? "enabled"
                    : "disabled"
              }
            />

            <Separator />

            <p className="text-xs leading-relaxed text-muted-foreground">
              Dum-E runs entirely on this machine via Ollama and sqlite-vec. Manual text is parsed
              locally with PyMuPDF (AGPL). When the internet fallback is used, only your question
              text is sent to your own local SearXNG instance — never filenames, file paths, or
              manual content, and never to a third-party API.
            </p>
          </div>
        ) : (
          <p className="text-sm text-muted-foreground">Couldn't load status.</p>
        )}
      </DialogContent>
    </Dialog>
  );
}
