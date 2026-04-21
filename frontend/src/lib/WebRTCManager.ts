import { ICE_CONFIG } from "./consts";
import type { Session } from "./interfaces/session";

interface WRTCManagerCallbacks {
  onRemoteTrack?: (track: MediaStreamTrack, streams: readonly MediaStream[]) => void;
  onICECandidate?: (session: Session, candidate: RTCIceCandidateInit) => void;
}

export class WRTCManager {
  private connections: Record<string, RTCPeerConnection> = {};
  private candidateQueues: Record<string, RTCIceCandidateInit[]> = {};
  private Callbacks: WRTCManagerCallbacks;

  constructor(callbacks: WRTCManagerCallbacks) {
    this.Callbacks = callbacks;
  }

  public finish() {
    for (const connectionId in this.connections) {
      this.closePeerConnection(WRTCManager.keyToSession(connectionId));
    }
  }

  public static sessionToKey(session: Session): string {
    return `${session.id}:${session.type}`;
  }

  public static keyToSession(key: string): Session {
    const parts = key.split(':');
    return { id: Number(parts[0]), type: Number(parts[1]) };
  }

  public createPeerConnection(connection: Session) {
    const key = WRTCManager.sessionToKey(connection);
    if (this.connections[key]) {
      this.connections[key].close();
    }

    const pc = new RTCPeerConnection(ICE_CONFIG);
    this.connections[key] = pc;
    this.candidateQueues[key] = [];

    pc.addEventListener('connectionstatechange', () => this.onConnectionState(connection, pc.connectionState));
    pc.addEventListener('track', ({ track, streams }) => this.Callbacks.onRemoteTrack?.(track, streams));
    pc.addEventListener('icecandidate', ({ candidate }) => {
      if (candidate) this.Callbacks.onICECandidate?.(connection, candidate.toJSON());
    });

    return pc;
  }

  private async onConnectionState(connection: Session, state: RTCPeerConnectionState) {
    switch (state) {
      case "closed":
      case "failed":
        this.closePeerConnection(connection);
        break;
      case "connected":
        this.flushCandidateQueue(connection);
        break;
      default:
        break;
    }
  }

  public closePeerConnection(session: Session) {
    const key = WRTCManager.sessionToKey(session);
    const pc = this.connections[key];
    if (!pc) return;
    pc.close();
    delete this.connections[key];
    delete this.candidateQueues[key];
  }

  public async addTrack(session: Session, track: MediaStreamTrack) {
    const key = WRTCManager.sessionToKey(session);
    const pc = this.connections[key];
    if (!pc) throw new Error("[AddTrack] PeerConnection not found for Session: " + key);
    pc.addTrack(track);
  }

  public async setStreamBandwidth(
    session: Session,
    maxBitrate: number,
    maxFramerate: number,
  ) {
    const key = WRTCManager.sessionToKey(session);
    const peerConnection = this.connections[key];
    if (!peerConnection) throw new Error("[SetStreamBandwidth] PeerConnection not found for ID: " + key);

    for (const sender of peerConnection.getSenders()) {
      if (sender.track?.kind !== "video") continue;
      const params = sender.getParameters();
      if (!params.encodings?.length) params.encodings = [{}];
      params.encodings[0].maxBitrate = maxBitrate;
      params.encodings[0].maxFramerate = maxFramerate;
      params.encodings[0].networkPriority = "high";
      params.encodings[0].active = true;
      params.encodings[0].priority = "high";
      params.encodings[0].scaleResolutionDownBy = 1.0;
      await sender.setParameters(params).catch(() => { });
    }
  }

  public getConnections() {
    return Object.keys(this.connections);
  }

  public getPeerConnection(session: Session) {
    return this.connections[WRTCManager.sessionToKey(session)];
  }

  public async addICECandidate(session: Session, candidate: RTCIceCandidateInit) {
    const key = WRTCManager.sessionToKey(session);
    const pc = this.connections[key];
    if (!pc || !pc.remoteDescription) {
      this.candidateQueues[key]?.push(candidate);
      return;
    }
    await pc.addIceCandidate(new RTCIceCandidate(candidate)).catch((err) => {
      console.warn(`[AddICECandidate] Failed to add ICE candidate for session: ${key}`, err);
    });
  }

  public async flushCandidateQueue(session: Session) {
    const key = WRTCManager.sessionToKey(session);
    const pc = this.connections[key];
    if (!pc || !pc.remoteDescription) return;
    const queue = this.candidateQueues[key] || [];
    for await (const candidate of queue) {
      await pc.addIceCandidate(new RTCIceCandidate(candidate)).catch(() => { });
    }
    this.candidateQueues[key] = [];
  }

  public async setRemoteDescription(session: Session, description: RTCSessionDescriptionInit) {
    const key = WRTCManager.sessionToKey(session);
    const pc = this.connections[key];
    if (!pc || (pc.currentLocalDescription !== null && pc.signalingState === "stable")) throw new Error("[SetRemoteDescription] PeerConnection not found or is already connected for ID: " + key);
    console.log(description);
    await pc.setRemoteDescription(new RTCSessionDescription(description));
    this.flushCandidateQueue(session);
  }

  public async createStreamOffer(session: Session) {
    const key = WRTCManager.sessionToKey(session);
    const pc = this.connections[key];
    if (!pc) throw new Error("[CreateStreamOffer] PeerConnection not found for ID: " + key);
    const offer = await pc.createOffer();
    await pc.setLocalDescription(offer);
    return offer;
  }

  public async createStreamAnswer(session: Session) {
    const key = WRTCManager.sessionToKey(session);
    const pc = this.connections[key];
    if (!pc) throw new Error("[CreateStreamAnswer] PeerConnection not found for ID: " + key);
    const answer = await pc.createAnswer();
    await pc.setLocalDescription(answer);
    return answer;
  }
}