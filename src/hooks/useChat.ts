import { listen } from "@tauri-apps/api/event";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "@/lib/api";
import type { ChatDoneEvent, ChatMessage, ChatTokenEvent } from "@/lib/types";

// Deterministic trigger for the b-roll scoring action, checked before the
// message ever reaches the LLM/RAG pipeline -- not model-driven tool-calling.
// A chat model isn't trusted to reliably decide when to fire an action
// (see PLAN.md's Decline-path lesson); a real "the model decides" tool-call
// path is a reasonable v2 once this pipeline itself is proven out.
function isBrollScoreRequest(text: string): boolean {
  const mentionsBroll = /\bb[- ]?roll\b/i.test(text);
  const mentionsScoring = /\b(score|scoring|analyze|analyse|rank|grade)\b/i.test(text);
  return mentionsBroll && mentionsScoring;
}

export function useChat() {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [streamingText, setStreamingText] = useState<string | null>(null);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const nextOptimisticId = useRef(-1);

  useEffect(() => {
    api
      .listMessages()
      .then(setMessages)
      .catch((e) => setError(String(e)));

    const unlistenToken = listen<ChatTokenEvent>("chat://token", (event) => {
      setStreamingText((prev) => (prev ?? "") + event.payload.delta);
    });
    const unlistenDone = listen<ChatDoneEvent>("chat://done", (event) => {
      setMessages((prev) => [...prev, event.payload.message]);
      setStreamingText(null);
      setPending(false);
    });
    const unlistenError = listen<string>("chat://error", (event) => {
      setError(event.payload);
      setStreamingText(null);
      setPending(false);
    });

    return () => {
      unlistenToken.then((f) => f());
      unlistenDone.then((f) => f());
      unlistenError.then((f) => f());
    };
  }, []);

  const send = useCallback(
    async (text: string) => {
      const trimmed = text.trim();
      if (!trimmed || pending) return;

      setError(null);
      const optimisticId = nextOptimisticId.current--;
      setMessages((prev) => [
        ...prev,
        {
          id: optimisticId,
          chat_id: 1,
          role: "user",
          content: trimmed,
          used_fallback: false,
          citations: [],
          created_at: new Date().toISOString(),
        },
      ]);

      if (isBrollScoreRequest(trimmed)) {
        setPending(true);
        const folder = await openDialog({ directory: true }).catch(() => null);
        if (typeof folder !== "string") {
          // Picker cancelled -- nothing was actually asked, so drop the
          // optimistic bubble rather than leaving an unanswered question.
          setMessages((prev) => prev.filter((m) => m.id !== optimisticId));
          setPending(false);
          return;
        }
        try {
          const reply = await api.scoreBrollFolder(trimmed, folder);
          setMessages((prev) => [...prev, reply]);
        } catch (e) {
          setError(String(e));
        } finally {
          setPending(false);
        }
        return;
      }

      setPending(true);
      setStreamingText("");

      try {
        await api.askQuestion(trimmed);
      } catch (e) {
        setError(String(e));
        setStreamingText(null);
        setPending(false);
      }
    },
    [pending],
  );

  const clear = useCallback(async () => {
    await api.clearChat();
    setMessages([]);
    setStreamingText(null);
    setError(null);
  }, []);

  return { messages, streamingText, pending, error, send, clear };
}
