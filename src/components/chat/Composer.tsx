import { Send } from "lucide-react";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";

export function Composer({
  onSend,
  disabled,
}: {
  onSend: (text: string) => void;
  disabled: boolean;
}) {
  const [text, setText] = useState("");

  const submit = () => {
    if (!text.trim() || disabled) return;
    onSend(text);
    setText("");
  };

  return (
    <div className="flex items-end gap-2 border-t border-border bg-background p-4">
      <Textarea
        value={text}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter" && !e.shiftKey) {
            e.preventDefault();
            submit();
          }
        }}
        placeholder="Ask Dum-E about your gear or editing software..."
        rows={1}
        className="max-h-40 min-h-11 flex-1 resize-none"
      />
      <Button size="icon" onClick={submit} disabled={disabled || !text.trim()} aria-label="Send">
        <Send className="size-4" />
      </Button>
    </div>
  );
}
