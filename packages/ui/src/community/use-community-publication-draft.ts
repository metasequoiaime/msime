import { useState, type Dispatch, type SetStateAction } from "react";
import { boundedGraphemes } from "../core/text";
import { randomUuid } from "../core/random-id";

export interface CommunityPublicationDraft {
  name: string;
  description: string;
  agreed: boolean;
  publicationId: string;
  setName: Dispatch<SetStateAction<string>>;
  setDescription: Dispatch<SetStateAction<string>>;
  setAgreed: Dispatch<SetStateAction<boolean>>;
  onNameChange: (value: string) => void;
  onDescriptionChange: (value: string) => void;
  onAgreedChange: (value: boolean) => void;
  resetPublication: () => void;
}

/** Shared publication metadata state and retry identity for community dialogs. */
export function useCommunityPublicationDraft(): CommunityPublicationDraft {
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [agreed, setAgreed] = useState(false);
  const [publicationId, setPublicationId] = useState(randomUuid);

  const resetPublication = () => setPublicationId(randomUuid());
  const onNameChange = (value: string) => {
    resetPublication();
    setName(boundedGraphemes(value, 32));
  };
  const onDescriptionChange = (value: string) => {
    resetPublication();
    setDescription(value);
  };

  return {
    name,
    description,
    agreed,
    publicationId,
    setName,
    setDescription,
    setAgreed,
    onNameChange,
    onDescriptionChange,
    onAgreedChange: setAgreed,
    resetPublication,
  };
}
