import { Tracker } from "./types";

// TODO: This needs to get refactored to have a typeguard for the tracker types
export function getTrackerURL(tracker: Tracker): string {
    if ('http' in tracker) {
        return tracker.http;
    } else if ('udp' in tracker) {
        return tracker.udp;
    } else if ('dht' in tracker) {
        return tracker.dht;
    }
    throw new Error('Unknown tracker type');
}