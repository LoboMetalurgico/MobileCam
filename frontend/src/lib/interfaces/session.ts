import type { SessionType } from "$lib/protos/common";

export interface Session {
  id: number,
  type: SessionType,
}