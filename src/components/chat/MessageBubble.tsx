import { Bot, Search, User } from "lucide-react";
import { cn } from "@/lib/utils";
import type { ChatMessage } from "@/lib/types";
import { CitationChips } from "./CitationChips";

export function MessageBubble({ message }: { message: ChatMessage }) {
  const isUser = message.role === "user";

  return (
    <div className={cn("flex gap-3", isUser && "flex-row-reverse")}>
      <div
        className={cn(
          "flex size-8 shrink-0 items-center justify-center rounded-full",
          isUser ? "bg-secondary text-secondary-foreground" : "bg-primary text-primary-foreground",
        )}
      >
        {isUser ? <User className="size-4" /> : <Bot className="size-4" />}
      </div>

      <div className={cn("flex max-w-[75%] flex-col gap-1", isUser && "items-end")}>
        {!isUser && (
          <div className="flex items-center gap-1.5 text-xs font-medium text-muted-foreground">
            Dum-E
            {message.used_fallback && (
              <span className="inline-flex items-center gap-1 text-amber-500 dark:text-amber-400">
                <Search className="size-3" />
                web search
              </span>
            )}
          </div>
        )}
        <div
          className={cn(
            "whitespace-pre-wrap rounded-2xl px-4 py-2.5 text-sm leading-relaxed",
            isUser
              ? "rounded-tr-sm bg-secondary text-secondary-foreground"
              : "rounded-tl-sm bg-card text-card-foreground border border-border",
          )}
        >
          {message.content}
        </div>
        {!isUser && <CitationChips citations={message.citations} />}
      </div>
    </div>
  );
}
