/**
 * Sends playbacks' reports one after the other: a player that closes still
 * gets its last position and its session's end to the server, in the
 * order they were made (progress, then `stopped`, then DELETE). Best
 * effort: a failure is dropped, as on the web and Android.
 */
export class Reporter {
  private tail: Promise<void> = Promise.resolve();

  send(report: () => Promise<unknown>): void {
    this.tail = this.tail.then(
      () => report().then(
        () => undefined,
        () => undefined,
      ),
      () => undefined,
    );
  }

  /** Resolves once everything queued so far went out, or after `timeoutMs`. */
  flush(timeoutMs: number): Promise<void> {
    return new Promise((resolve) => {
      const timer = setTimeout(resolve, timeoutMs);
      this.tail.then(() => {
        clearTimeout(timer);
        resolve();
      });
    });
  }
}
