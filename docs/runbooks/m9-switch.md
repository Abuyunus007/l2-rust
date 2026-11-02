# Runbook: Switch from Java to Rust (M9-S01)

The end of M9 (D-57): the Rust game server ("Kain") takes the place of the Java game server
("Sieghardt"), and the Rust server does every global job the Java server did. This is guided by
the **Setup view, section "6. Switch to Rust (M9)"** of the admin GUI; this runbook is the same
thing by hand, plus the way back. Do it when both servers have been tested side by side
(runbooks m1 to m8) and nobody needs the Java server any more.

Nothing changes without your click. The assistant refuses to write while a Java game server is
registered at the login server, and it only writes after a finished backup.

## 1. Prepare

1. **Backup (M9-H01).** GUI: Backups view "Backup now", or the assistant's "Back up the database
   now" (it waits until the backup is done and ok). Check the file exists in the backup folder.
   Never start the switch without one: the way back (section 4) may need it.
2. **Stop the Java game server.** Stop `javagame` in the Servers view (or close its window). Wait
   until the login server's list shows it down: Servers view, game server table, Sieghardt is not
   "registered" / status "Down". The assistant's "Ask the login server" shows the same list and
   refuses to go on while a game server other than the Rust one is registered.
   Do not stop it by killing a process by name; use the GUI or close the Java window.
3. **Write down** the ports and the server id you have now (the assistant prints them):
   - Rust game server (side by side): `GameserverPort = 7778`, `RequestServerID = 3`
     (`game/config/Server.ini`).
   - Java game server: port 7777, id 2.
   - The `switch-before.ini` the assistant writes into the run folder keeps the nine old values;
     a copy on paper does no harm.
4. Make sure no player is online on the Rust server (announce it, `//shutdown`, or wait).

## 2. Switch

Both choices set `GameserverPort = 7777`, so clients keep using the address they know.

**Choice A: take Sieghardt's id (2), the assistant's default.** `RequestServerID = 2`.
- The login server already has a `gameservers` row for id 2 (Java's `hexid`), so the Rust server
  must send the same secret: copy the Java server's `hexid.txt` (`ServerID=2`, `HexID=...`) into the
  Rust server's `game/config/hexid.txt`. Without it the Rust server cannot prove it owns id 2 and the
  login server does not accept it (the login server refuses a known id with a wrong secret).
- Players see the same server entry as before (name and id).

**Choice B: keep id 3.** Leave `RequestServerID = 3`; `hexid.txt` and the `gameservers` row of
id 3 stay as they are. The server list then shows Kain; Sieghardt's entry stays down. The client's
server name in the list comes from the login server's `servername.xml`, per id.

Then, in `game/config/SideBySide.ini`, all seven switches go to **True**:
`DailyResetDatabaseWrites`, `ClanMaintenanceDatabaseWrites`, `MailExpiryDatabaseWrites`,
`FishingChampionshipDatabaseWrites`, `CycleDatabaseWrites`, `OfflineTradeDatabaseWrites`,
`SiegeDatabaseWrites` (D-49, D-51..D-54). From now on the Rust server writes the daily resets,
clan and mail maintenance, the cycles (Olympiad, Seven Signs, grand bosses), siege results and
offline-shop rows. This is only safe because the Java server is down and stays down.

GUI: step 2 of the assistant shows each key, its value now and what changes, then "Apply the
switch". By hand: edit the values in place and keep everything else.

Finally restart the Rust game server (Servers view) so it reads the files.

## 3. Check

After the restart, with a test account:
1. The login server's list shows the Rust server up on port 7777 (and Sieghardt down).
2. Log in, choose a character (a character that was last online on Sieghardt), enter the world.
3. Walk, fight, loot, use a skill, and log out and in again: the character is where it was.
4. Mail and the clan warehouse show the items as before (D-50 re-reads).
5. Siege, Olympiad and Seven Signs pages (and `//checkolympiad`) show the state the Java server
   left.
6. Within the first minutes the log shows the global jobs running (daily reset at its time,
   clan and mail maintenance); no log line says a switch is off.
7. A player who logs out with an open store stays as an offline trader; the account is freed
   (D-52 with the switch True).
8. Keep the backup until a few days have passed without problems.

## 4. Way back

1. Stop the Rust game server (Servers view).
2. **Back to side by side:** GUI, step 3 of the assistant, "Back to side by side". It writes the
   values from `switch-before.ini` back (switches, port, id), byte for byte as they were, and
   removes the file. By hand: all seven switches False, `GameserverPort = 7778`,
   `RequestServerID = 3` (or what you wrote down). If you copied the Java `hexid.txt` for choice A,
   put the Rust server's own back.
3. If the data is wrong (the Rust server wrote something you do not want), **restore the backup**:
   the Backups view shows the restore command for the chosen file. Stop both game servers first.
   Everything done after the backup is lost.
4. Start the Java game server (Servers view, `javagame`), wait until the login server's list shows
   it up, then start the Rust server again if you want the side-by-side set-up back.
