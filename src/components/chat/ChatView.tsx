import { Bot } from "lucide-react";
import { useEffect, useRef } from "react";
import { ScrollArea } from "@/components/ui/scroll-area";
import { useChat } from "@/hooks/useChat";
import { Composer } from "./Composer";
import { MessageBubble } from "./MessageBubble";

export function ChatView() {
  const { messages, streamingText, pending, error, send } = useChat();
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages, streamingText]);

  return (
    <div className="flex h-full flex-col">
      <ScrollArea className="min-h-0 flex-1">
        <div className="mx-auto flex max-w-3xl flex-col gap-5 p-6">
          {messages.length === 0 && !pending && (
            <div className="flex flex-col items-center gap-3 py-16 text-center text-muted-foreground">
              <div className="flex size-14 items-center justify-center rounded-full bg-primary text-primary-foreground">
                <Bot className="size-7" />
              </div>
              <p className="max-w-sm text-sm">
                Hi, I&apos;m Dum-E! Ask me anything about your cameras, monitors, audio gear, or
                DaVinci Resolve &mdash; I&apos;ll check the manuals first.
              </p>
            </div>
          )}

          {messages.map((message) => (
            <MessageBubble key={message.id} message={message} />
          ))}

          {pending && (
            <MessageBubble
              message={{
                id: -1,
                chat_id: 1,
                role: "assistant",
                content: streamingText || "...",
                used_fallback: false,
                citations: [],
                created_at: new Date().toISOString(),
              }}
            />
          )}

          {error && (
            <p className="rounded-lg border border-destructive/30 bg-destructive/10 px-4 py-2 text-sm text-destructive">
              {error}
            </p>
          )}

          <div ref={bottomRef} />
        </div>
      </ScrollArea>

      <div className="mx-auto w-full max-w-3xl">
        <Composer onSend={send} disabled={pending} />
      </div>
    </div>
  );
}
