package l2rust.vectors;

import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.math.BigInteger;
import java.nio.file.Path;
import java.security.KeyFactory;
import java.security.interfaces.RSAPublicKey;
import java.security.spec.RSAPublicKeySpec;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Random;

import org.l2jmobius.commons.network.ConnectionConfig;
import org.l2jmobius.commons.network.buffer.WriteBuffer;
import org.l2jmobius.commons.network.packet.SimpleWritablePacket;
import org.l2jmobius.commons.network.pool.ResourcePool;
import org.l2jmobius.gameserver.LoginServerThread;
import org.l2jmobius.gameserver.network.loginserverpackets.send.AuthRequest;
import org.l2jmobius.gameserver.network.loginserverpackets.send.BlowFishKey;
import org.l2jmobius.gameserver.network.loginserverpackets.send.ChangeAccessLevel;
import org.l2jmobius.gameserver.network.loginserverpackets.send.ChangePassword;
import org.l2jmobius.gameserver.network.loginserverpackets.send.PlayerAuthRequest;
import org.l2jmobius.gameserver.network.loginserverpackets.send.PlayerInGame;
import org.l2jmobius.gameserver.network.loginserverpackets.send.PlayerLogout;
import org.l2jmobius.gameserver.network.loginserverpackets.send.PlayerTracert;
import org.l2jmobius.gameserver.network.loginserverpackets.send.ReplyCharacters;
import org.l2jmobius.gameserver.network.loginserverpackets.send.SendMail;
import org.l2jmobius.gameserver.network.loginserverpackets.send.ServerStatus;
import org.l2jmobius.gameserver.network.loginserverpackets.send.TempBan;
import org.l2jmobius.loginserver.LoginServer;
import org.l2jmobius.loginserver.controller.SessionKey;
import org.l2jmobius.loginserver.enums.AccountKickedReason;
import org.l2jmobius.loginserver.enums.LoginFailReason;
import org.l2jmobius.loginserver.enums.PlayFailReason;
import org.l2jmobius.loginserver.network.LoginClient;
import org.l2jmobius.loginserver.network.gameserverpackets.send.ChangePasswordResponse;
import org.l2jmobius.loginserver.network.gameserverpackets.send.InitLS;
import org.l2jmobius.loginserver.network.gameserverpackets.send.KickPlayer;
import org.l2jmobius.loginserver.network.gameserverpackets.send.LoginServerFail;
import org.l2jmobius.loginserver.network.gameserverpackets.send.PlayerAuthResponse;
import org.l2jmobius.loginserver.network.gameserverpackets.send.RequestCharacters;
import org.l2jmobius.loginserver.network.serverpackets.AccountKicked;
import org.l2jmobius.loginserver.network.serverpackets.GGAuth;
import org.l2jmobius.loginserver.network.serverpackets.Init;
import org.l2jmobius.loginserver.network.serverpackets.LoginFail;
import org.l2jmobius.loginserver.network.serverpackets.LoginOk;
import org.l2jmobius.loginserver.network.serverpackets.LoginServerPacket;
import org.l2jmobius.loginserver.network.serverpackets.PlayFail;
import org.l2jmobius.loginserver.network.serverpackets.PlayOk;

/**
 * Login packet vectors for M1-JV-02 (D-41): the plaintext payload of every login-server packet (client side) and of every login server <-> game server packet.
 * <p>
 * Payload = the bytes after the 2-byte length header and before any encryption, opcode included. The framing (length header, checksum, Blowfish/XOR) is covered by {@code crypto_*.json}.
 * <p>
 * How the bytes are produced (same rules as {@link CryptoVectors}: no clock, no {@code Rnd}, fixed seeds):
 * <ul>
 * <li>{@code SimpleWritablePacket} subclasses (login server <-> game server): the real class is constructed and {@code getSendableBytes()} is called; the 2-byte header is checked against the length and removed.</li>
 * <li>{@code LoginServerPacket} subclasses (login server -> client): the real class is constructed and its {@code write(client, buffer)} runs on a real {@link WriteBuffer} positioned after the header, exactly like {@code WritablePacket.writeDataToBuffer}. These packets never touch the client, so {@code null} is passed.</li>
 * <li>{@code ServerList} can't be constructed without a running {@code GameServerTable} and a {@code LoginClient}. Its constructor is skipped (instance allocated without running it, state injected by reflection) and the real {@code writeImpl} produces the bytes. What is NOT covered is the constructor logic (status computation, GM-only filtering).</li>
 * <li>{@code AuthResponse} calls {@code GameServerTable.getInstance()}, whose constructor needs a database. It is produced by <b>replaying</b> its three write calls on an anonymous {@code SimpleWritablePacket}; the variants say so.</li>
 * <li>{@code TempBan} writes {@code System.currentTimeMillis()}. The real class runs, the expiry is range-checked against the clock, then the 8 bytes are replaced by a fixed value.</li>
 * </ul>
 */
public final class LoginPacketVectors
{
	private static final ResourcePool POOL = new ResourcePool();

	/** The "current time" TempBan vectors pretend to have (2023-11-14T22:13:20Z). */
	private static final long FIXED_NOW_MILLIS = 1_700_000_000_000L;

	// Strings that exercise the UTF-16LE writer. Unicode escapes keep the source file ASCII.
	private static final String CYRILLIC = "Привет"; // "Privet".
	private static final String KOREAN = "한글이름"; // Hangul syllables.
	private static final String EMOJI = "a😀b"; // Surrogate pair: 4 UTF-8 bytes, 2 UTF-16 code units.
	private static final String QUOTES = "q\"b\\s\tn\n";

	private LoginPacketVectors()
	{
	}

	public static void generate(Path outDir) throws Exception
	{
		final List<Object> packets = new ArrayList<>();
		loginServerToClient(packets);
		loginServerToGameServer(packets);
		gameServerToLoginServer(packets);

		final List<Object> skipped = new ArrayList<>();
		skipped.add(skip("Init(LoginClient)", "Constructor that reads scrambled modulus, Blowfish key and session id from a LoginClient. Same bytes as Init(byte[], byte[], int), which is covered."));
		skipped.add(skip("ServerList pending-delete timers", "The per-server delete times are written as (deleteTime - System.currentTimeMillis()) / 1000, so the bytes depend on the clock. Only the branches without timers are covered. Cover with a capture."));
		skipped.add(skip("ServerList(LoginClient) constructor logic", "Status computation (GM-only, access level < 0 gives DOWN), GM-only filtering and the 500 ms wait for the character counts need GameServerTable, a database and a LoginClient. Only the wire layout is covered. Cover with a capture."));
		skipped.add(skip("BlowFishKey with a null/empty key or a null public key", "Throws IllegalArgumentException, nothing is written."));
		skipped.add(skip("BlowFishKey when the RSA operation fails", "Logs and leaves the packet empty (no opcode). Not reachable with a valid key."));

		final Map<String, Object> root = Json.header("org.l2jmobius.loginserver.network.serverpackets.*, org.l2jmobius.loginserver.network.gameserverpackets.send.*, org.l2jmobius.gameserver.network.loginserverpackets.send.*");
		root.put("note", "payload = packet bytes with opcode, WITHOUT the 2-byte length header and WITHOUT encryption/checksum. Integers in fields are signed Java values, byte arrays are hex strings (keys ending in _hex), strings are the Java strings (non-ASCII and surrogate pairs included; the wire format is UTF-16LE code units with a 0x0000 terminator). A variant with produced_by = replay was not made by the real class (see its what). All other variants come from the real Java class.");
		root.put("packets", packets);
		root.put("skipped", skipped);
		Json.write(outDir.resolve("login_packets.json"), root);
	}

	// ------------------------------------------------------------------------------------------------
	// Login server -> game client (LoginServerPacket, plain WriteBuffer).
	// ------------------------------------------------------------------------------------------------

	private static void loginServerToClient(List<Object> out) throws Exception
	{
		final Random random = new Random(0x4C320101L);

		// Init: opcode 0x00. Protocol revision 0xc621 and the four GG ints are constants in Java.
		final List<Object> init = new ArrayList<>();
		addInit(init, "session 0x12345678, ascending key bytes", 0x12345678, sequence(128, 0), sequence(16, 0x10));
		addInit(init, "negative session id, random key bytes", -1, bytes(random, 128), bytes(random, 16));
		addInit(init, "Integer.MIN_VALUE session, all 0xff scrambled key, all 0x00 Blowfish key", Integer.MIN_VALUE, filled(128, 0xFF), new byte[16]);
		addInit(init, "session 0 with a shorter Blowfish key (length is not checked in Java)", 0, bytes(random, 128), bytes(random, 8));
		out.add(packet("Init", "org.l2jmobius.loginserver.network.serverpackets.Init", 0x00, "login_server_to_client", init));

		// LoginFail: opcode 0x01, one byte reason. Every enum value.
		final List<Object> loginFail = new ArrayList<>();
		for (LoginFailReason reason : LoginFailReason.values())
		{
			loginFail.add(variant(reason.name(), fields("reason", reason.getCode()), lsPacket(new LoginFail(reason))));
		}
		out.add(packet("LoginFail", "org.l2jmobius.loginserver.network.serverpackets.LoginFail", 0x01, "login_server_to_client", loginFail));

		// AccountKicked: opcode 0x02, int reason. Every enum value.
		final List<Object> kicked = new ArrayList<>();
		for (AccountKickedReason reason : AccountKickedReason.values())
		{
			kicked.add(variant(reason.name(), fields("reason", reason.getCode()), lsPacket(new AccountKicked(reason))));
		}
		out.add(packet("AccountKicked", "org.l2jmobius.loginserver.network.serverpackets.AccountKicked", 0x02, "login_server_to_client", kicked));

		// LoginOk: opcode 0x03. Session key constructor order is (loginOk1, loginOk2, playOk1, playOk2).
		final List<Object> loginOk = new ArrayList<>();
		addLoginOk(loginOk, "ascending ints", 0x01020304, 0x05060708, 0x090a0b0c, 0x0d0e0f10);
		addLoginOk(loginOk, "negative ints", -1, -2, Integer.MIN_VALUE, -123456789);
		addLoginOk(loginOk, "zeros and max", 0, Integer.MAX_VALUE, 0, Integer.MAX_VALUE);
		addLoginOk(loginOk, "random ints", random.nextInt(), random.nextInt(), random.nextInt(), random.nextInt());
		out.add(packet("LoginOk", "org.l2jmobius.loginserver.network.serverpackets.LoginOk", 0x03, "login_server_to_client", loginOk));

		// ServerList: opcode 0x04 (see serverListVariants).
		out.add(packet("ServerList", "org.l2jmobius.loginserver.network.serverpackets.ServerList", 0x04, "login_server_to_client", serverListVariants()));

		// PlayFail: opcode 0x06, one byte reason. Every enum value.
		final List<Object> playFail = new ArrayList<>();
		for (PlayFailReason reason : PlayFailReason.values())
		{
			playFail.add(variant(reason.name(), fields("reason", reason.getCode()), lsPacket(new PlayFail(reason))));
		}
		out.add(packet("PlayFail", "org.l2jmobius.loginserver.network.serverpackets.PlayFail", 0x06, "login_server_to_client", playFail));

		// PlayOk: opcode 0x07. Constructor order of SessionKey is (loginOk1, loginOk2, playOk1, playOk2); PlayOk writes playOk1, playOk2.
		final List<Object> playOk = new ArrayList<>();
		addPlayOk(playOk, "ascending ints", 0x01020304, 0x05060708, 0x090a0b0c, 0x0d0e0f10);
		addPlayOk(playOk, "negative ints", 1, 2, -1, Integer.MIN_VALUE);
		addPlayOk(playOk, "random ints", random.nextInt(), random.nextInt(), random.nextInt(), random.nextInt());
		out.add(packet("PlayOk", "org.l2jmobius.loginserver.network.serverpackets.PlayOk", 0x07, "login_server_to_client", playOk));

		// GGAuth: opcode 0x0b, response int + four zero ints.
		final List<Object> ggAuth = new ArrayList<>();
		for (int response : new int[]
		{
			0x0b,
			0,
			-1,
			Integer.MAX_VALUE,
			Integer.MIN_VALUE
		})
		{
			ggAuth.add(variant("response " + response, fields("response", response), lsPacket(new GGAuth(response))));
		}
		out.add(packet("GGAuth", "org.l2jmobius.loginserver.network.serverpackets.GGAuth", 0x0b, "login_server_to_client", ggAuth));
	}

	private static void addInit(List<Object> variants, String what, int sessionId, byte[] publicKey, byte[] blowfishKey) throws Exception
	{
		final byte[] payload = lsPacket(new Init(publicKey, blowfishKey, sessionId));
		final Map<String, Object> f = fields("session_id", sessionId, "public_key_hex", publicKey, "blowfish_key_hex", blowfishKey);
		f.put("protocol_revision", 0xc621); // Constant inside Init.
		variants.add(variant(what, f, payload));
	}

	private static void addLoginOk(List<Object> variants, String what, int loginOk1, int loginOk2, int playOk1, int playOk2) throws Exception
	{
		final byte[] payload = lsPacket(new LoginOk(new SessionKey(loginOk1, loginOk2, playOk1, playOk2)));
		variants.add(variant(what, fields("login_ok1", loginOk1, "login_ok2", loginOk2), payload));
	}

	private static void addPlayOk(List<Object> variants, String what, int loginOk1, int loginOk2, int playOk1, int playOk2) throws Exception
	{
		final byte[] payload = lsPacket(new PlayOk(new SessionKey(loginOk1, loginOk2, playOk1, playOk2)));
		variants.add(variant(what, fields("play_ok1", playOk1, "play_ok2", playOk2), payload));
	}

	// ------------------------------------------------------------------------------------------------
	// ServerList: real writeImpl, instance built without its constructor.
	// ------------------------------------------------------------------------------------------------

	private static List<Object> serverListVariants() throws Exception
	{
		final List<Object> variants = new ArrayList<>();

		variants.add(serverList("one server, 2 characters on it", 1, //
			List.of(server(1, new int[]
			{
				127,
				0,
				0,
				1
			}, 7777, 0, true, 12, 3000, false, 2, 1)), //
			chars(1, 2), false));

		variants.add(serverList("no servers, no character info (null map)", 0, List.of(), null, false));

		variants.add(serverList("no servers, empty character map", 0, List.of(), chars(), false));

		variants.add(serverList("two servers, second is DOWN, ages 15/18, brackets, type flags 0x41, characters listed out of order, empty delete-timer arrays", 2, //
			List.of(server(1, new int[]
			{
				192,
				168,
				1,
				10
			}, 7777, 15, false, 1, 100, true, 1, 0x41), //
				server(2, new int[]
				{
					10,
					0,
					0,
					254
				}, 7778, 18, true, 0xFFFF, 0xFFFF, true, org.l2jmobius.loginserver.network.gameserverpackets.receive.ServerStatus.STATUS_DOWN, 8)), //
			chars(2, 1, 1, 3), true));

		variants.add(serverList("server id 255, ip 255.255.255.255 (bytes are masked with 0xff), max port, last server 255, 255 characters", 255, //
			List.of(server(255, new int[]
			{
				255,
				255,
				255,
				255
			}, 65535, 0, false, 0, 0, false, 0, 0x7F)), //
			chars(255, 255), false));

		variants.add(serverList("three servers with status values 1, 3, 5 (only DOWN is written as 0), mixed player counts", 3, //
			List.of(server(3, new int[]
			{
				1,
				2,
				3,
				4
			}, 2106, 0, true, 1, 2, false, 1, 1), //
				server(4, new int[]
				{
					5,
					6,
					7,
					8
				}, 2107, 0, false, 30000, 40000, true, 3, 2), //
				server(5, new int[]
				{
					9,
					10,
					11,
					12
				}, 2108, 15, true, 0, 1, false, 5, 4)), //
			chars(3, 0, 4, 9, 5, 1), false));

		return variants;
	}

	/** Server description used both as the JSON field and as input of the reflection injection. */
	private static Map<String, Object> server(int id, int[] ip, int port, int ageLimit, boolean pvp, int currentPlayers, int maxPlayers, boolean brackets, int status, int serverType)
	{
		return fields("server_id", id, "ip", Arrays.stream(ip).boxed().toList(), "port", port, "age_limit", ageLimit, "pvp", pvp, "current_players", currentPlayers, "max_players", maxPlayers, "brackets", brackets, "status", status, "server_type", serverType);
	}

	/** Ordered (server id, character count) pairs, flattened. */
	private static Map<Integer, Integer> chars(int... pairs)
	{
		final Map<Integer, Integer> map = new LinkedHashMap<>(); // Order matters on the wire; Java iterates its HashMap in this order for small ids.
		for (int i = 0; i < pairs.length; i += 2)
		{
			map.put(pairs[i], pairs[i + 1]);
		}
		return map;
	}

	@SuppressWarnings("unchecked")
	private static Map<String, Object> serverList(String what, int lastServer, List<Map<String, Object>> servers, Map<Integer, Integer> charsOnServers, boolean emptyDeleteTimers) throws Exception
	{
		final Class<?> listClass = Class.forName("org.l2jmobius.loginserver.network.serverpackets.ServerList");
		final Class<?> dataClass = Class.forName("org.l2jmobius.loginserver.network.serverpackets.ServerList$ServerData");
		final Object list = allocateInstance(listClass);

		final List<Object> serverData = new ArrayList<>();
		for (Map<String, Object> s : servers)
		{
			final Object d = allocateInstance(dataClass);
			final List<Integer> ip = (List<Integer>) s.get("ip");
			final byte[] ipBytes = new byte[4];
			for (int i = 0; i < 4; i++)
			{
				ipBytes[i] = (byte) (int) ip.get(i);
			}
			set(dataClass, d, "_ip", ipBytes);
			set(dataClass, d, "_port", s.get("port"));
			set(dataClass, d, "_ageLimit", s.get("age_limit"));
			set(dataClass, d, "_pvp", s.get("pvp"));
			set(dataClass, d, "_currentPlayers", s.get("current_players"));
			set(dataClass, d, "_maxPlayers", s.get("max_players"));
			set(dataClass, d, "_brackets", s.get("brackets"));
			set(dataClass, d, "_status", s.get("status"));
			set(dataClass, d, "_serverId", s.get("server_id"));
			set(dataClass, d, "_serverType", s.get("server_type"));
			serverData.add(d);
		}
		set(listClass, list, "_servers", serverData);
		set(listClass, list, "_lastServer", lastServer);
		set(listClass, list, "_charsOnServers", charsOnServers);
		if (emptyDeleteTimers && (charsOnServers != null))
		{
			// Servers that have an entry with zero timers: writes a 0 count and no ints, no clock involved.
			final Map<Integer, long[]> toDelete = new LinkedHashMap<>();
			for (Integer id : charsOnServers.keySet())
			{
				if ((id % 2) == 0)
				{
					toDelete.put(id, new long[0]);
				}
			}
			set(listClass, list, "_charsToDelete", toDelete);
		}

		final Method writeImpl = listClass.getDeclaredMethod("writeImpl", LoginClient.class, WriteBuffer.class);
		writeImpl.setAccessible(true);
		final WriteBuffer buffer = new WriteBuffer(POOL, listClass);
		buffer.position(ConnectionConfig.HEADER_SIZE);
		writeImpl.invoke(list, null, buffer);
		buffer.mark();
		final byte[] payload = stripHeader(buffer.toByteArray());
		buffer.releaseResources();

		final Map<String, Object> f = fields("last_server", lastServer, "servers", new ArrayList<Object>(servers));
		if (charsOnServers == null)
		{
			f.put("chars_on_servers", null);
		}
		else
		{
			final List<Object> entries = new ArrayList<>();
			for (Map.Entry<Integer, Integer> e : charsOnServers.entrySet())
			{
				entries.add(fields("server_id", e.getKey(), "chars", e.getValue()));
			}
			f.put("chars_on_servers", entries);
		}
		return variant(what, f, payload);
	}

	private static void set(Class<?> type, Object target, String name, Object value) throws Exception
	{
		final Field field = type.getDeclaredField(name);
		field.setAccessible(true);
		field.set(target, value);
	}

	/**
	 * Creates an instance without running any constructor. Goes through reflection so javac doesn't print the "internal proprietary API" warning (build.ps1 treats stderr output as an error).
	 */
	private static Object allocateInstance(Class<?> type) throws Exception
	{
		final Class<?> unsafeClass = Class.forName("sun.misc.Unsafe");
		final Field f = unsafeClass.getDeclaredField("theUnsafe");
		f.setAccessible(true);
		return unsafeClass.getMethod("allocateInstance", Class.class).invoke(f.get(null), type);
	}

	// ------------------------------------------------------------------------------------------------
	// Login server -> game server (SimpleWritablePacket).
	// ------------------------------------------------------------------------------------------------

	private static void loginServerToGameServer(List<Object> out) throws Exception
	{
		final Random random = new Random(0x4C320102L);
		final String d = "login_server_to_game_server";

		// InitLS: opcode 0x00, int protocol revision, int key length, key bytes.
		final List<Object> initLs = new ArrayList<>();
		addInitLs(initLs, "65-byte modulus with leading 0x00 (BigInteger.toByteArray of a 512-bit key)", concat(new byte[1], bytes(random, 64)));
		addInitLs(initLs, "64-byte modulus", bytes(random, 64));
		addInitLs(initLs, "empty key (length is not checked)", new byte[0]);
		addInitLs(initLs, "ascending 128 bytes", sequence(128, 0));
		out.add(packet("InitLS", "org.l2jmobius.loginserver.network.gameserverpackets.send.InitLS", 0x00, d, initLs));

		// LoginServerFail: opcode 0x01, one byte reason.
		final List<Object> fail = new ArrayList<>();
		final int[] reasons =
		{
			LoginServerFail.REASON_IP_BANNED,
			LoginServerFail.REASON_IP_RESERVED,
			LoginServerFail.REASON_WRONG_HEXID,
			LoginServerFail.REASON_ID_RESERVED,
			LoginServerFail.REASON_NO_FREE_ID,
			LoginServerFail.NOT_AUTHED,
			LoginServerFail.REASON_ALREADY_LOGGED8IN
		};
		for (int reason : reasons)
		{
			fail.add(variant("reason " + reason, fields("reason", reason), simple(new LoginServerFail(reason))));
		}
		fail.add(variant("NOTE(java-quirk): reason 0x1ff, only the low byte is written", fields("reason", 0x1ff), simple(new LoginServerFail(0x1ff))));
		out.add(packet("LoginServerFail", "org.l2jmobius.loginserver.network.gameserverpackets.send.LoginServerFail", 0x01, d, fail));

		// AuthResponse: opcode 0x02, byte server id, string name (replayed, see class comment).
		final List<Object> authResponse = new ArrayList<>();
		addAuthResponse(authResponse, "server 1, ASCII name", 1, "Bartz");
		addAuthResponse(authResponse, "server 127, non-ASCII name", 127, CYRILLIC + KOREAN);
		addAuthResponse(authResponse, "server 255, emoji name (surrogate pair)", 255, EMOJI);
		addAuthResponse(authResponse, "NOTE(java-quirk): GameServerTable.getServerNameById returns null for an unknown id, writeString(null) writes only the terminator", 9, null);
		addAuthResponse(authResponse, "server id 0x101, only the low byte is written, empty name", 0x101, "");
		out.add(packet("AuthResponse", "org.l2jmobius.loginserver.network.gameserverpackets.send.AuthResponse", 0x02, d, authResponse));

		// PlayerAuthResponse: opcode 0x03, string account, byte result.
		final List<Object> playerAuthResponse = new ArrayList<>();
		playerAuthResponse.add(variant("ok", fields("account", "player1", "response", true), simple(new PlayerAuthResponse("player1", true))));
		playerAuthResponse.add(variant("denied, non-ASCII account", fields("account", CYRILLIC, "response", false), simple(new PlayerAuthResponse(CYRILLIC, false))));
		playerAuthResponse.add(variant("empty account", fields("account", "", "response", true), simple(new PlayerAuthResponse("", true))));
		playerAuthResponse.add(variant("emoji account", fields("account", EMOJI, "response", false), simple(new PlayerAuthResponse(EMOJI, false))));
		out.add(packet("PlayerAuthResponse", "org.l2jmobius.loginserver.network.gameserverpackets.send.PlayerAuthResponse", 0x03, d, playerAuthResponse));

		// KickPlayer: opcode 0x04, string account.
		out.add(packet("KickPlayer", "org.l2jmobius.loginserver.network.gameserverpackets.send.KickPlayer", 0x04, d, accountVariants(KickPlayer::new)));

		// RequestCharacters: opcode 0x05, string account.
		out.add(packet("RequestCharacters", "org.l2jmobius.loginserver.network.gameserverpackets.send.RequestCharacters", 0x05, d, accountVariants(RequestCharacters::new)));

		// ChangePasswordResponse: opcode 0x06, string character name, string message.
		final List<Object> changePasswordResponse = new ArrayList<>();
		changePasswordResponse.add(variant("ASCII", fields("character_name", "Hero", "message", "Password changed."), simple(new ChangePasswordResponse("Hero", "Password changed."))));
		changePasswordResponse.add(variant("non-ASCII", fields("character_name", KOREAN, "message", CYRILLIC), simple(new ChangePasswordResponse(KOREAN, CYRILLIC))));
		changePasswordResponse.add(variant("empty strings", fields("character_name", "", "message", ""), simple(new ChangePasswordResponse("", ""))));
		changePasswordResponse.add(variant("null strings (writeString(null) writes only the terminator)", fields("character_name", null, "message", null), simple(new ChangePasswordResponse(null, null))));
		out.add(packet("ChangePasswordResponse", "org.l2jmobius.loginserver.network.gameserverpackets.send.ChangePasswordResponse", 0x06, d, changePasswordResponse));
	}

	private static void addInitLs(List<Object> variants, String what, byte[] key) throws Exception
	{
		final Map<String, Object> f = fields("public_key_hex", key);
		f.put("protocol_revision", LoginServer.PROTOCOL_REV);
		variants.add(variant(what, f, simple(new InitLS(key))));
	}

	/** Replays the three write calls of {@code AuthResponse}; the real constructor needs {@code GameServerTable.getInstance()}. */
	private static void addAuthResponse(List<Object> variants, String what, int serverId, String serverName) throws Exception
	{
		final SimpleWritablePacket replay = new SimpleWritablePacket()
		{
			{
				writeByte(0x02);
				writeByte(serverId);
				writeString(serverName);
			}
		};
		final Map<String, Object> v = variant(what, fields("server_id", serverId, "server_name", serverName), simple(replay));
		v.put("produced_by", "replay");
		variants.add(v);
	}

	private interface StringPacket
	{
		SimpleWritablePacket create(String account);
	}

	private static List<Object> accountVariants(StringPacket factory) throws Exception
	{
		final List<Object> variants = new ArrayList<>();
		for (String account : new String[]
		{
			"player1",
			CYRILLIC,
			KOREAN,
			EMOJI,
			QUOTES,
			"",
			null
		})
		{
			final String what = account == null ? "null (writeString(null) writes only the terminator)" : account.isEmpty() ? "empty" : "account " + account.length() + " chars";
			variants.add(variant(what, fields("account", account), simple(factory.create(account))));
		}
		return variants;
	}

	// ------------------------------------------------------------------------------------------------
	// Game server -> login server (SimpleWritablePacket).
	// ------------------------------------------------------------------------------------------------

	private static void gameServerToLoginServer(List<Object> out) throws Exception
	{
		final Random random = new Random(0x4C320103L);
		final String d = "game_server_to_login_server";

		// BlowFishKey: opcode 0x00, int length, RSA/ECB/nopadding encrypted Blowfish key. Uses the RSA-512 key of crypto_rsa_nopadding.json (same seed).
		final List<Object> blowFishKey = new ArrayList<>();
		final Rsa rsa512 = rsaKey(512, 0x4C320007L);
		for (int length : new int[]
		{
			16,
			22,
			40,
			63
		})
		{
			final byte[] key = bytes(random, length);
			key[0] &= 0x3F; // Keep the integer below the modulus, so the raw RSA result is reversible.
			final Map<String, Object> f = fields("blowfish_key_hex", key, "rsa_n_hex", unsigned(rsa512.n), "rsa_e", 65537);
			blowFishKey.add(variant("Blowfish key of " + length + " bytes, RSA-512 public key", f, simple(new BlowFishKey(key, rsa512.pub))));
		}
		out.add(packet("BlowFishKey", "org.l2jmobius.gameserver.network.loginserverpackets.send.BlowFishKey", 0x00, d, blowFishKey));

		// AuthRequest: opcode 0x01.
		final List<Object> authRequest = new ArrayList<>();
		addAuthRequest(authRequest, "typical: server 1, one subnet/host pair", 1, false, bytes(random, 16), 7777, false, 5000, List.of("0.0.0.0/0"), List.of("127.0.0.1"));
		addAuthRequest(authRequest, "flags on, max values, two pairs", 255, true, filled(16, 0xFF), 65535, true, Integer.MAX_VALUE, List.of("192.168.0.0/16", "10.0.0.0/8"), List.of("192.168.1.2", "game." + CYRILLIC + ".example"));
		addAuthRequest(authRequest, "negative max players, short hex id, no pairs", 2, false, bytes(random, 3), 0, false, -1, List.of(), List.of());
		addAuthRequest(authRequest, "null hex id/subnets/hosts become empty", 3, true, null, 2106, false, 0, null, null);
		addAuthRequest(authRequest, "NOTE(java-quirk): lists of different size, only min(size) pairs are written; null entries become empty strings", 4, false, bytes(random, 16), 7777, true, 100, java.util.Arrays.asList("a", null, "c"), java.util.Arrays.asList("x", "y"));
		addAuthRequest(authRequest, "id 0x1ff and port 0x1ffff: byte and short are truncated", 0x1ff, false, bytes(random, 16), 0x1ffff, false, 1, List.of(), List.of());
		out.add(packet("AuthRequest", "org.l2jmobius.gameserver.network.loginserverpackets.send.AuthRequest", 0x01, d, authRequest));

		// PlayerInGame: opcode 0x02, short count, strings.
		final List<Object> playerInGame = new ArrayList<>();
		playerInGame.add(variant("single player (String constructor)", fields("players", List.of("player1")), simple(new PlayerInGame("player1"))));
		playerInGame.add(variant("single null player becomes one empty name", fields("players", List.of("")), simple(new PlayerInGame((String) null))));
		playerInGame.add(variant("list of three, non-ASCII", fields("players", List.of("a", CYRILLIC, KOREAN)), simple(new PlayerInGame(List.of("a", CYRILLIC, KOREAN)))));
		playerInGame.add(variant("empty list", fields("players", List.of()), simple(new PlayerInGame(List.of()))));
		playerInGame.add(variant("null list", fields("players", null), simple(new PlayerInGame((List<String>) null))));
		playerInGame.add(variant("list with a null entry written as empty", fields("players", java.util.Arrays.asList("a", null)), simple(new PlayerInGame(java.util.Arrays.asList("a", null)))));
		out.add(packet("PlayerInGame", "org.l2jmobius.gameserver.network.loginserverpackets.send.PlayerInGame", 0x02, d, playerInGame));

		// PlayerLogout: opcode 0x03, string.
		out.add(packet("PlayerLogout", "org.l2jmobius.gameserver.network.loginserverpackets.send.PlayerLogout", 0x03, d, accountVariants(PlayerLogout::new)));

		// ChangeAccessLevel: opcode 0x04, int access, string player.
		final List<Object> changeAccessLevel = new ArrayList<>();
		changeAccessLevel.add(variant("level 100", fields("player", "gm1", "access", 100), simple(new ChangeAccessLevel("gm1", 100))));
		changeAccessLevel.add(variant("banned level -100, non-ASCII", fields("player", CYRILLIC, "access", -100), simple(new ChangeAccessLevel(CYRILLIC, -100))));
		changeAccessLevel.add(variant("Integer.MIN_VALUE, empty name", fields("player", "", "access", Integer.MIN_VALUE), simple(new ChangeAccessLevel("", Integer.MIN_VALUE))));
		changeAccessLevel.add(variant("Integer.MAX_VALUE, null name", fields("player", null, "access", Integer.MAX_VALUE), simple(new ChangeAccessLevel(null, Integer.MAX_VALUE))));
		out.add(packet("ChangeAccessLevel", "org.l2jmobius.gameserver.network.loginserverpackets.send.ChangeAccessLevel", 0x04, d, changeAccessLevel));

		// PlayerAuthRequest: opcode 0x05, string account, playOk1, playOk2, loginOk1, loginOk2.
		final List<Object> playerAuthRequest = new ArrayList<>();
		addPlayerAuthRequest(playerAuthRequest, "ascending ints", "player1", 0x01020304, 0x05060708, 0x090a0b0c, 0x0d0e0f10, false);
		addPlayerAuthRequest(playerAuthRequest, "negative ints, non-ASCII account", CYRILLIC, -1, Integer.MIN_VALUE, -2, Integer.MAX_VALUE, false);
		addPlayerAuthRequest(playerAuthRequest, "null account and null key become empty string and zeros", null, 0, 0, 0, 0, true);
		out.add(packet("PlayerAuthRequest", "org.l2jmobius.gameserver.network.loginserverpackets.send.PlayerAuthRequest", 0x05, d, playerAuthRequest));

		// ServerStatus: opcode 0x06, int count, (int id, int value) pairs.
		final List<Object> serverStatus = new ArrayList<>();
		serverStatus.add(serverStatus("no attributes"));
		serverStatus.add(serverStatus("status and max players", ServerStatus.SERVER_LIST_STATUS, ServerStatus.STATUS_GOOD, ServerStatus.MAX_PLAYERS, 3000));
		serverStatus.add(serverStatus("all five attributes", ServerStatus.SERVER_LIST_STATUS, ServerStatus.STATUS_GM_ONLY, ServerStatus.SERVER_TYPE, ServerStatus.SERVER_NORMAL | ServerStatus.SERVER_EVENT, ServerStatus.SERVER_LIST_SQUARE_BRACKET, ServerStatus.ON, ServerStatus.MAX_PLAYERS, Integer.MAX_VALUE, ServerStatus.SERVER_AGE, ServerStatus.SERVER_AGE_18));
		serverStatus.add(serverStatus("negative and repeated attributes", ServerStatus.MAX_PLAYERS, -1, ServerStatus.MAX_PLAYERS, Integer.MIN_VALUE, -5, 7));
		final int[] many = new int[40]; // 20 pairs: more than the initial capacity of 16 pairs, the array grows.
		for (int i = 0; i < 20; i++)
		{
			many[i * 2] = (i % 5) + 1;
			many[(i * 2) + 1] = i * 1000;
		}
		serverStatus.add(serverStatus("20 attributes (array grows past 16 pairs)", many));
		out.add(packet("ServerStatus", "org.l2jmobius.gameserver.network.loginserverpackets.send.ServerStatus", 0x06, d, serverStatus));

		// PlayerTracert: opcode 0x07, six strings.
		final List<Object> tracert = new ArrayList<>();
		tracert.add(tracertVariant("ASCII", "player1", "192.168.0.5", "10.0.0.1", "10.0.1.1", "10.0.2.1", "10.0.3.1"));
		tracert.add(tracertVariant("non-ASCII", CYRILLIC, KOREAN, EMOJI, QUOTES, "", "x"));
		tracert.add(tracertVariant("all null become empty", null, null, null, null, null, null));
		tracert.add(tracertVariant("only account", "player1", "", "", "", "", ""));
		out.add(packet("PlayerTracert", "org.l2jmobius.gameserver.network.loginserverpackets.send.PlayerTracert", 0x07, d, tracert));

		// ReplyCharacters: opcode 0x08, string account, byte chars, byte count, long[] times.
		final List<Object> reply = new ArrayList<>();
		reply.add(variant("no pending deletes", fields("account", "player1", "chars", 3, "times", List.of()), simple(new ReplyCharacters("player1", 3, List.of()))));
		reply.add(variant("three timers incl. negative and max", fields("account", CYRILLIC, "chars", 7, "times", List.of(1L, Long.MAX_VALUE, -1L)), simple(new ReplyCharacters(CYRILLIC, 7, List.of(1L, Long.MAX_VALUE, -1L)))));
		reply.add(variant("null account, chars 0x1ff (low byte only), null list", fields("account", null, "chars", 0x1ff, "times", null), simple(new ReplyCharacters(null, 0x1ff, null))));
		reply.add(variant("timer list with a null entry written as 0", fields("account", EMOJI, "chars", 255, "times", java.util.Arrays.asList(1_700_000_000_000L, null, Long.MIN_VALUE)), simple(new ReplyCharacters(EMOJI, 255, java.util.Arrays.asList(1_700_000_000_000L, null, Long.MIN_VALUE)))));
		out.add(packet("ReplyCharacters", "org.l2jmobius.gameserver.network.loginserverpackets.send.ReplyCharacters", 0x08, d, reply));

		// SendMail: opcode 0x09, string account, string mail id, byte count, string args.
		final List<Object> sendMail = new ArrayList<>();
		sendMail.add(variant("two args", fields("account", "player1", "mail_id", "welcome", "args", List.of("a", "b")), simple(new SendMail("player1", "welcome", "a", "b"))));
		sendMail.add(variant("no args", fields("account", CYRILLIC, "mail_id", KOREAN, "args", List.of()), simple(new SendMail(CYRILLIC, KOREAN))));
		sendMail.add(variant("nulls", fields("account", null, "mail_id", null, "args", null), simple(new SendMail(null, null, (String[]) null))));
		sendMail.add(variant("null arg written as empty", fields("account", "a", "mail_id", "m", "args", java.util.Arrays.asList("x", null, EMOJI)), simple(new SendMail("a", "m", "x", null, EMOJI))));
		out.add(packet("SendMail", "org.l2jmobius.gameserver.network.loginserverpackets.send.SendMail", 0x09, d, sendMail));

		// TempBan: opcode 0x0a, string account, string ip, long expiry, byte 0. Expiry = now + minutes * 60000.
		final List<Object> tempBan = new ArrayList<>();
		tempBan.add(tempBanVariant("5 minutes", "player1", "10.1.2.3", 5L));
		tempBan.add(tempBanVariant("non-ASCII account, empty ip, 1 day", CYRILLIC, "", 1440L));
		tempBan.add(tempBanVariant("0 minutes", "a", "b", 0L));
		tempBan.add(tempBanVariant("negative minutes are clamped to 0", "a", "b", -10L));
		tempBan.add(tempBanVariant("null account and ip, 1 minute", null, null, 1L));
		tempBan.add(tempBanVariant("NOTE(java-quirk): Long.MAX_VALUE minutes overflows minutes * 60000", "a", "b", Long.MAX_VALUE));
		out.add(packet("TempBan", "org.l2jmobius.gameserver.network.loginserverpackets.send.TempBan", 0x0a, d, tempBan));

		// ChangePassword: opcode 0x0b, four strings.
		final List<Object> changePassword = new ArrayList<>();
		changePassword.add(changePasswordVariant("ASCII", "acc", "Hero", "old", "new"));
		changePassword.add(changePasswordVariant("non-ASCII", CYRILLIC, KOREAN, EMOJI, QUOTES));
		changePassword.add(changePasswordVariant("empty", "", "", "", ""));
		changePassword.add(changePasswordVariant("null", null, null, null, null));
		out.add(packet("ChangePassword", "org.l2jmobius.gameserver.network.loginserverpackets.send.ChangePassword", 0x0b, d, changePassword));
	}

	private static void addAuthRequest(List<Object> variants, String what, int id, boolean acceptAlternate, byte[] hexId, int port, boolean reserveHost, int maxPlayers, List<String> subnets, List<String> hosts) throws Exception
	{
		final Map<String, Object> f = fields("id", id, "accept_alternate", acceptAlternate, "hex_id_hex", hexId, "port", port, "reserve_host", reserveHost, "max_players", maxPlayers, "subnets", subnets, "hosts", hosts);
		variants.add(variant(what, f, simple(new AuthRequest(id, acceptAlternate, hexId, port, reserveHost, maxPlayers, subnets, hosts))));
	}

	private static void addPlayerAuthRequest(List<Object> variants, String what, String account, int loginOk1, int loginOk2, int playOk1, int playOk2, boolean nullKey) throws Exception
	{
		final LoginServerThread.SessionKey key = nullKey ? null : new LoginServerThread.SessionKey(loginOk1, loginOk2, playOk1, playOk2);
		final Map<String, Object> f = fields("account", account);
		if (nullKey)
		{
			f.put("session_key", null);
		}
		else
		{
			f.put("play_ok1", playOk1);
			f.put("play_ok2", playOk2);
			f.put("login_ok1", loginOk1);
			f.put("login_ok2", loginOk2);
		}
		variants.add(variant(what, f, simple(new PlayerAuthRequest(account, key))));
	}

	private static Map<String, Object> serverStatus(String what, int... pairs) throws Exception
	{
		final ServerStatus status = new ServerStatus();
		final List<Object> attributes = new ArrayList<>();
		for (int i = 0; i < pairs.length; i += 2)
		{
			status.addAttribute(pairs[i], pairs[i + 1]);
			attributes.add(fields("id", pairs[i], "value", pairs[i + 1]));
		}
		return variant(what, fields("attributes", attributes), simple(status));
	}

	private static Map<String, Object> tracertVariant(String what, String account, String pcIp, String hop1, String hop2, String hop3, String hop4) throws Exception
	{
		return variant(what, fields("account", account, "pc_ip", pcIp, "hop1", hop1, "hop2", hop2, "hop3", hop3, "hop4", hop4), simple(new PlayerTracert(account, pcIp, hop1, hop2, hop3, hop4)));
	}

	private static Map<String, Object> changePasswordVariant(String what, String account, String character, String oldPass, String newPass) throws Exception
	{
		return variant(what, fields("account", account, "character", character, "old_pass", oldPass, "new_pass", newPass), simple(new ChangePassword(account, character, oldPass, newPass)));
	}

	/** Runs the real TempBan, range-checks the expiry it wrote against the clock, then pins the clock to {@link #FIXED_NOW_MILLIS}. */
	private static Map<String, Object> tempBanVariant(String what, String account, String ip, long minutes) throws Exception
	{
		final long before = System.currentTimeMillis();
		final byte[] real = simple(new TempBan(account, ip, minutes));
		final long after = System.currentTimeMillis();

		final long clamped = minutes > 0L ? minutes : 0L;
		final long offset = clamped * 60000L; // Wraps on overflow exactly like Java.
		// Layout: opcode, account (UTF-16LE + 0x0000), ip (UTF-16LE + 0x0000), long, 0x00.
		final int expiryAt = real.length - 9;
		long expiry = 0;
		for (int i = 7; i >= 0; i--)
		{
			expiry = (expiry << 8) | (real[expiryAt + i] & 0xFF);
		}
		check((expiry >= (before + offset)) && (expiry <= (after + offset)), "TempBan expiry " + expiry + " not within [" + (before + offset) + ", " + (after + offset) + "]");
		check(real[real.length - 1] == 0, "TempBan trailing byte");

		final byte[] payload = real.clone();
		final long fixedExpiry = FIXED_NOW_MILLIS + offset;
		for (int i = 0; i < 8; i++)
		{
			payload[expiryAt + i] = (byte) (fixedExpiry >> (8 * i));
		}
		final Map<String, Object> f = fields("account", account, "ip", ip, "minutes", minutes, "now_millis", FIXED_NOW_MILLIS, "expire_millis", fixedExpiry);
		final Map<String, Object> v = variant(what, f, payload);
		v.put("clock_pinned", true);
		return v;
	}

	// ------------------------------------------------------------------------------------------------
	// Shared helpers.
	// ------------------------------------------------------------------------------------------------

	/** Writes a LoginServerPacket the way WritablePacket.writeDataToBuffer does, minus the client. Returns payload without the header slot. */
	private static byte[] lsPacket(LoginServerPacket packet) throws Exception
	{
		final Method write = LoginServerPacket.class.getDeclaredMethod("write", LoginClient.class, WriteBuffer.class);
		write.setAccessible(true);
		final WriteBuffer buffer = new WriteBuffer(POOL, packet.getClass());
		buffer.position(ConnectionConfig.HEADER_SIZE);
		check((Boolean) write.invoke(packet, null, buffer), packet + ".write returned false");
		buffer.mark();
		final byte[] all = buffer.toByteArray();
		buffer.releaseResources();
		return stripHeader(all);
	}

	private static byte[] stripHeader(byte[] all)
	{
		check(all.length >= ConnectionConfig.HEADER_SIZE, "packet shorter than the header");
		return Arrays.copyOfRange(all, ConnectionConfig.HEADER_SIZE, all.length);
	}

	/** Real SimpleWritablePacket output: checks the 2-byte little-endian length header and removes it. */
	private static byte[] simple(SimpleWritablePacket packet)
	{
		final byte[] sendable = packet.getSendableBytes();
		check(sendable != null, packet + " wrote nothing");
		final int declared = (sendable[0] & 0xFF) | ((sendable[1] & 0xFF) << 8);
		check(declared == sendable.length, packet + " header " + declared + " != length " + sendable.length);
		return stripHeader(sendable);
	}

	private static Map<String, Object> variant(String what, Map<String, Object> fields, byte[] payload)
	{
		final Map<String, Object> v = Json.obj();
		v.put("what", what);
		v.put("fields", fields);
		v.put("payload", payload);
		return v;
	}

	private static Map<String, Object> packet(String name, String javaClass, int opcode, String direction, List<Object> variants)
	{
		check(variants.size() >= 2, name + " needs at least 2 variants");
		for (Object o : variants)
		{
			final byte[] payload = (byte[]) ((Map<?, ?>) o).get("payload");
			check((payload.length > 0) && ((payload[0] & 0xFF) == opcode), name + " payload does not start with opcode " + opcode);
		}
		final Map<String, Object> p = Json.obj();
		p.put("name", name);
		p.put("java_class", javaClass);
		p.put("direction", direction);
		p.put("opcode", opcode);
		p.put("variants", variants);
		return p;
	}

	private static Map<String, Object> skip(String name, String reason)
	{
		final Map<String, Object> s = Json.obj();
		s.put("name", name);
		s.put("reason", reason);
		return s;
	}

	/** Ordered map from alternating names and values. */
	private static Map<String, Object> fields(Object... namesAndValues)
	{
		final Map<String, Object> m = Json.obj();
		for (int i = 0; i < namesAndValues.length; i += 2)
		{
			m.put((String) namesAndValues[i], namesAndValues[i + 1]);
		}
		return m;
	}

	private static byte[] bytes(Random random, int length)
	{
		final byte[] b = new byte[length];
		random.nextBytes(b);
		return b;
	}

	private static byte[] sequence(int length, int start)
	{
		final byte[] b = new byte[length];
		for (int i = 0; i < length; i++)
		{
			b[i] = (byte) (start + i);
		}
		return b;
	}

	private static byte[] filled(int length, int value)
	{
		final byte[] b = new byte[length];
		Arrays.fill(b, (byte) value);
		return b;
	}

	private static byte[] concat(byte[] a, byte[] b)
	{
		final byte[] r = Arrays.copyOf(a, a.length + b.length);
		System.arraycopy(b, 0, r, a.length, b.length);
		return r;
	}

	private static void check(boolean ok, String message)
	{
		if (!ok)
		{
			throw new IllegalStateException("vector self-check failed: " + message);
		}
	}

	/** Big-endian bytes of a positive number without the sign byte. */
	private static byte[] unsigned(BigInteger value)
	{
		final byte[] b = value.toByteArray();
		return b[0] == 0 ? Arrays.copyOfRange(b, 1, b.length) : b;
	}

	// Deterministic RSA key with e = 65537. Same algorithm and seed as CryptoVectors.rsaKey(512, 0x4C320007L), so the key matches crypto_rsa_nopadding.json.
	private static final class Rsa
	{
		final BigInteger n;
		final RSAPublicKey pub;

		Rsa(BigInteger n, BigInteger e) throws Exception
		{
			this.n = n;
			pub = (RSAPublicKey) KeyFactory.getInstance("RSA").generatePublic(new RSAPublicKeySpec(n, e));
		}
	}

	private static Rsa rsaKey(int bits, long seed) throws Exception
	{
		final Random random = new Random(seed);
		final BigInteger e = BigInteger.valueOf(65537);
		while (true)
		{
			final BigInteger p = BigInteger.probablePrime(bits / 2, random);
			final BigInteger q = BigInteger.probablePrime(bits / 2, random);
			if (p.equals(q))
			{
				continue;
			}
			final BigInteger n = p.multiply(q);
			final BigInteger phi = p.subtract(BigInteger.ONE).multiply(q.subtract(BigInteger.ONE));
			if ((n.bitLength() != bits) || !phi.gcd(e).equals(BigInteger.ONE))
			{
				continue;
			}
			return new Rsa(n, e);
		}
	}
}
