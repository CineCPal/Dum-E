import { BookOpen, Globe } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import type { Citation } from "@/lib/types";

export function CitationChips({ citations }: { citations: Citation[] }) {
  if (citations.length === 0) return null;

  return (
    <div className="mt-2 flex flex-wrap gap-1.5">
      {citations.map((citation, i) =>
        citation.kind === "manual" ? (
          <Badge
            key={i}
            variant="secondary"
            className="gap-1 border-primary/20 bg-primary/10 text-primary-foreground/90 dark:text-primary"
          >
            <BookOpen className="size-3" />
            {citation.title} · p.{citation.page_start}
            {citation.page_end !== citation.page_start ? `-${citation.page_end}` : ""}
          </Badge>
        ) : (
          <a key={i} href={citation.url} target="_blank" rel="noreferrer">
            <Badge variant="outline" className="gap-1 hover:bg-accent">
              <Globe className="size-3" />
              {citation.title}
            </Badge>
          </a>
        ),
      )}
    </div>
  );
}
