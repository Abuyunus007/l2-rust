package l2rust.vectors;

import java.lang.reflect.Field;
import java.math.BigInteger;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.file.Path;
import java.security.KeyFactory;
import java.security.KeyPair;
import java.security.interfaces.RSAPrivateKey;
import java.security.interfaces.RSAPublicKey;
import java.security.spec.RSAPrivateKeySpec;
import java.security.spec.RSAPublicKeySpec;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Map;
import java.util.Random;

import javax.crypto.Cipher;

import org.l2jmobius.commons.crypt.BlowfishEngine;
import org.l2jmobius.commons.crypt.NewCrypt;
import org.l2jmobius.commons.network.buffer.ReadBuffer;
import org.l2jmobius.commons.network.buffer.WriteBuffer;
import org.l2jmobius.commons.network.pool.ResourcePool;
import org.l2jmobius.gameserver.network.BlowFishKeygen;
import org.l2jmobius.gameserver.network.Encryption;
import org.l2jmobius.loginserver.controller.ScrambledKeyPair;
import org.l2jmobius.loginserver.network.LoginEncryption;

/**
 * Crypto vectors for M1 (D-41). Every value comes from the real L2JMobius classes.
 * <p>
 * Rules for generators (copy them in new generators):
 * <ul>
 * <li>No {@code Rnd}, no {@code SecureRandom}, no clock. All inputs come from explicit constants or {@code new Random(seed)}.</li>
 * <li>Bytes are written as lowercase hex strings (see {@link Json#hex}). Integers are plain JSON numbers (signed Java {@code int}).</li>
 * <li>Every file starts with {@code source} and {@code generated_by} ({@link Json#header}).</li>
 * <li>Where the Java class can't be driven deterministically, the generator replays the same steps with the real building blocks and <b>checks the replay against the real class</b> before it writes anything.</li>
 * </ul>
 */
public final class CryptoVectors
{
	private static final byte[] NO_BYTES = new byte[0];

	private static final ResourcePool POOL = new ResourcePool();

	private CryptoVectors()
	{
	}

	public static void generate(Path outDir) throws Exception
	{
		blowfish(outDir);
		newCrypt(outDir);
		loginEncryption(outDir);
		scrambledModulus(outDir);
		rsaNoPadding(outDir);
		gameXor(outDir);
	}

	// ------------------------------------------------------------------------------------------------
	// Helpers.
	// ------------------------------------------------------------------------------------------------

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

	private static Map<String, Object> pair(String inName, byte[] in, String outName, byte[] out)
	{
		final Map<String, Object> o = Json.obj();
		o.put(inName, in);
		o.put(outName, out);
		return o;
	}

	private static byte[] getStaticField(Class<?> type, String name) throws Exception
	{
		final Field f = type.getDeclaredField(name);
		f.setAccessible(true);
		return ((byte[]) f.get(null)).clone();
	}

	private static byte[] getInstanceBytes(Object target, String name) throws Exception
	{
		final Field f = target.getClass().getDeclaredField(name);
		f.setAccessible(true);
		return ((byte[]) f.get(target)).clone();
	}

	private static void check(boolean ok, String message)
	{
		if (!ok)
		{
			throw new IllegalStateException("vector self-check failed: " + message);
		}
	}

	/** Runs the real {@code LoginEncryption.encrypt} on a pooled {@link WriteBuffer} and returns the encrypted packet. */
	private static byte[] loginEncrypt(LoginEncryption crypt, byte[] payload)
	{
		final WriteBuffer buffer = new WriteBuffer(POOL, LoginEncryption.class);
		buffer.writeBytes(payload);
		crypt.encrypt(buffer, 0, payload.length);
		final byte[] result = buffer.toByteArray();
		buffer.releaseResources();
		return result;
	}

	/** Runs the real {@code Encryption.encrypt} on a pooled {@link WriteBuffer}; returns the buffer contents afterwards. */
	private static byte[] gameEncrypt(Encryption crypt, byte[] data)
	{
		final WriteBuffer buffer = new WriteBuffer(POOL, Encryption.class);
		buffer.writeBytes(data);
		buffer.limit(data.length);
		crypt.encrypt(buffer, 0, data.length);
		final byte[] result = buffer.toByteArray();
		buffer.releaseResources();
		return result;
	}

	/** Runs the real {@code Encryption.decrypt} on a {@link ReadBuffer}; returns the buffer contents afterwards. */
	private static byte[] gameDecrypt(Encryption crypt, byte[] data)
	{
		final byte[] copy = data.clone();
		crypt.decrypt(ReadBuffer.of(ByteBuffer.wrap(copy).order(ByteOrder.LITTLE_ENDIAN)), 0, copy.length);
		return copy;
	}

	// ------------------------------------------------------------------------------------------------
	// crypto_blowfish.json
	// ------------------------------------------------------------------------------------------------

	private static void blowfish(Path outDir) throws Exception
	{
		final Random random = new Random(0x4C320001L);

		// The 22-byte key is the shape of the LS<->GS key text (21 chars + NUL); the content is made up.
		final byte[] key22 = Arrays.copyOf(bytes(random, 21), 22);
		final byte[][] keys =
		{
			getStaticField(LoginEncryption.class, "STATIC_BLOWFISH_KEY"), // 16 bytes, the real static login key.
			bytes(random, 16),
			key22,
			bytes(random, 40),
		};

		final List<Object> keyCases = new ArrayList<>();
		for (byte[] key : keys)
		{
			final BlowfishEngine engine = new BlowfishEngine();
			engine.init(key);

			final List<byte[]> plain = new ArrayList<>();
			plain.add(new byte[8]);
			plain.add(filled(8, 0xFF));
			plain.add(sequence(8, 0));
			plain.add(sequence(8, 0xF8));
			for (int i = 0; i < 4; i++)
			{
				plain.add(bytes(random, 8));
			}

			final List<Object> encrypt = new ArrayList<>();
			final List<byte[]> cipher = new ArrayList<>();
			for (byte[] block : plain)
			{
				final byte[] work = block.clone();
				engine.encryptBlock(work, 0);
				cipher.add(work);
				encrypt.add(pair("input", block, "output", work));
			}

			// Decrypt the encrypt outputs (round trip) plus blocks that were never produced by encryptBlock.
			final List<byte[]> cipherInputs = new ArrayList<>(cipher);
			cipherInputs.add(new byte[8]);
			cipherInputs.add(filled(8, 0xFF));
			cipherInputs.add(bytes(random, 8));
			cipherInputs.add(bytes(random, 8));
			final List<Object> decrypt = new ArrayList<>();
			for (byte[] block : cipherInputs)
			{
				final byte[] work = block.clone();
				engine.decryptBlock(work, 0);
				decrypt.add(pair("input", block, "output", work));
			}

			final Map<String, Object> c = Json.obj();
			c.put("key", key);
			c.put("encrypt_block", encrypt);
			c.put("decrypt_block", decrypt);
			keyCases.add(c);
		}

		final Map<String, Object> root = Json.header("org.l2jmobius.commons.crypt.BlowfishEngine#init/encryptBlock/decryptBlock");
		root.put("note", "Blocks are 8 bytes. The first key is the static login key from LoginEncryption.STATIC_BLOWFISH_KEY. Keys of 16, 16, 22 (incl. trailing NUL) and 40 bytes.");
		root.put("cases", keyCases);
		Json.write(outDir.resolve("crypto_blowfish.json"), root);
	}

	// ------------------------------------------------------------------------------------------------
	// crypto_newcrypt.json
	// ------------------------------------------------------------------------------------------------

	private static void newCrypt(Path outDir) throws Exception
	{
		final Random random = new Random(0x4C320002L);

		// appendChecksum: the last 4 bytes of the input are the (zero) slot that gets the checksum.
		final List<Object> append = new ArrayList<>();
		final List<Object> verify = new ArrayList<>();
		for (int size : new int[]
		{
			4,
			8,
			12,
			16,
			24,
			32,
			48,
			64,
			100
		})
		{
			final byte[] input = bytes(random, size);
			Arrays.fill(input, size - 4, size, (byte) 0);
			final byte[] output = input.clone();
			NewCrypt.appendChecksum(output);
			append.add(pair("input", input, "output", output));

			verify.add(verifyCase(output)); // Valid.
			if (size >= 8)
			{
				final byte[] flipData = output.clone();
				flipData[0] ^= 0x01;
				verify.add(verifyCase(flipData)); // Corrupted data.

				final byte[] flipSum = output.clone();
				flipSum[size - 1] ^= (byte) 0x80;
				verify.add(verifyCase(flipSum)); // Corrupted checksum.
			}
		}

		// All-zero packet: the checksum of zeros is zero, so this is valid.
		verify.add(verifyCase(new byte[8]));
		// Size rules: not a multiple of 4, size 4 (only a checksum), empty. All false in Java.
		verify.add(verifyCase(bytes(random, 6)));
		verify.add(verifyCase(bytes(random, 10)));
		verify.add(verifyCase(bytes(random, 4)));
		verify.add(verifyCase(NO_BYTES));

		// encXORPass.
		final List<Object> xor = new ArrayList<>();
		final int[] xorKeys =
		{
			random.nextInt(),
			0,
			-1,
			Integer.MIN_VALUE,
			Integer.MAX_VALUE,
			random.nextInt(),
			random.nextInt(),
			random.nextInt()
		};
		final int[] xorSizes =
		{
			12,
			16,
			16,
			24,
			24,
			32,
			40,
			64
		};
		for (int i = 0; i < xorSizes.length; i++)
		{
			final byte[] input = bytes(random, xorSizes[i]);
			final byte[] output = input.clone();
			NewCrypt.encXORPass(output, xorKeys[i]);
			final Map<String, Object> c = Json.obj();
			c.put("xor_key", xorKeys[i]);
			c.put("input", input);
			c.put("output", output);
			xor.add(c);
		}

		// crypt / decrypt, multi-block ECB.
		final List<Object> blowfish = new ArrayList<>();
		final byte[] key16 = bytes(random, 16);
		final byte[] key22 = Arrays.copyOf(bytes(random, 21), 22);
		for (byte[] key : new byte[][]
		{
			key16,
			key22
		})
		{
			final NewCrypt crypt = new NewCrypt(key);
			final List<Object> encrypt = new ArrayList<>();
			final List<Object> decrypt = new ArrayList<>();
			for (int blocks : new int[]
			{
				1,
				2,
				5
			})
			{
				final byte[] plain = bytes(random, blocks * 8);
				final byte[] enc = plain.clone();
				crypt.crypt(enc, 0, enc.length);
				encrypt.add(pair("input", plain, "output", enc));

				final byte[] cipher = bytes(random, blocks * 8);
				final byte[] dec = cipher.clone();
				crypt.decrypt(dec, 0, dec.length);
				decrypt.add(pair("input", cipher, "output", dec));
			}
			final Map<String, Object> c = Json.obj();
			c.put("key", key);
			c.put("crypt", encrypt);
			c.put("decrypt", decrypt);
			blowfish.add(c);
		}

		final Map<String, Object> root = Json.header("org.l2jmobius.commons.crypt.NewCrypt");
		root.put("note", "append_checksum: the input's last 4 bytes are the checksum slot (zero). verify_checksum: 'valid' is the return value of verifyChecksum(data). xor_pass: encXORPass(data, xor_key) with xor_key as signed int. blowfish: NewCrypt(key).crypt/decrypt over whole 8-byte blocks.");
		root.put("append_checksum", append);
		root.put("verify_checksum", verify);
		root.put("xor_pass", xor);
		root.put("blowfish", blowfish);
		Json.write(outDir.resolve("crypto_newcrypt.json"), root);
	}

	private static Map<String, Object> verifyCase(byte[] data)
	{
		final Map<String, Object> c = Json.obj();
		c.put("data", data);
		c.put("valid", NewCrypt.verifyChecksum(data));
		return c;
	}

	// ------------------------------------------------------------------------------------------------
	// crypto_login_encryption.json
	// ------------------------------------------------------------------------------------------------

	/** Same arithmetic as the private {@code LoginEncryption.encryptedSize}; checked against the real class below. */
	private static int loginPacketSize(int dataSize, boolean staticKey)
	{
		int size = dataSize + (staticKey ? 8 : 4);
		size += 8 - (size % 8);
		return size + 8;
	}

	/**
	 * The first packet of {@code LoginEncryption.encrypt} (static key). Java calls {@code Rnd.nextInt()} for the XOR key, which can't be injected, so this repeats the exact steps
	 * with the given key: zero-padded buffer, {@code NewCrypt.encXORPass}, then the static-key {@code NewCrypt.crypt}.
	 */
	private static byte[] loginFirstPacketReplay(byte[] payload, int xorKey, NewCrypt staticCrypt)
	{
		final int packetSize = loginPacketSize(payload.length, true);
		final byte[] raw = new byte[packetSize];
		System.arraycopy(payload, 0, raw, 0, payload.length);
		NewCrypt.encXORPass(raw, 0, packetSize, xorKey);
		staticCrypt.crypt(raw, 0, packetSize);
		return raw;
	}

	private static int readIntLe(byte[] b, int offset)
	{
		return (b[offset] & 0xFF) | ((b[offset + 1] & 0xFF) << 8) | ((b[offset + 2] & 0xFF) << 16) | ((b[offset + 3] & 0xFF) << 24);
	}

	private static void loginEncryption(Path outDir) throws Exception
	{
		final Random random = new Random(0x4C320003L);
		final byte[] staticKey = getStaticField(LoginEncryption.class, "STATIC_BLOWFISH_KEY");
		final NewCrypt staticCrypt = new NewCrypt(staticKey);

		// First packet (static key).
		final List<Object> first = new ArrayList<>();
		for (int size = 1; size <= 40; size++)
		{
			final byte[] payload = bytes(random, size);
			final int xorKey = random.nextInt();

			// Self-check: run the real class (random XOR key), recover that key from its output, replay with it, and compare.
			final byte[] real = loginEncrypt(new LoginEncryption(), payload);
			check(real.length == loginPacketSize(size, true), "static packet size for payload " + size);
			final byte[] decrypted = real.clone();
			staticCrypt.decrypt(decrypted, 0, decrypted.length);
			final byte[] padded = Arrays.copyOf(payload, real.length);
			final int plainBlock = readIntLe(padded, 4);
			final int recoveredKey = (readIntLe(decrypted, 4) ^ plainBlock) - plainBlock;
			check(Arrays.equals(real, loginFirstPacketReplay(payload, recoveredKey, staticCrypt)), "static packet replay for payload " + size);

			final byte[] output = loginFirstPacketReplay(payload, xorKey, staticCrypt);
			final Map<String, Object> c = Json.obj();
			c.put("payload", payload);
			c.put("xor_key", xorKey);
			c.put("output", output);
			first.add(c);
		}

		// Session phase: the first encrypt() call (static, random) is spent on a dummy packet; later calls use the session key.
		final byte[] sessionKey = bytes(random, 16);
		final LoginEncryption sessionCrypt = new LoginEncryption();
		sessionCrypt.setKey(sessionKey);
		loginEncrypt(sessionCrypt, new byte[]
		{
			0
		});

		final List<Object> session = new ArrayList<>();
		for (int size = 1; size <= 40; size++)
		{
			final byte[] payload = bytes(random, size);
			final byte[] encrypted = loginEncrypt(sessionCrypt, payload);
			check(encrypted.length == loginPacketSize(size, false), "session packet size for payload " + size);

			// Decrypt with a fresh instance (the real decrypt path: Blowfish, then checksum check, written back only when valid).
			final LoginEncryption decryptor = new LoginEncryption();
			decryptor.setKey(sessionKey);
			final byte[] plain = encrypted.clone();
			final boolean valid = decryptor.decrypt(ReadBuffer.of(ByteBuffer.wrap(plain).order(ByteOrder.LITTLE_ENDIAN)), 0, plain.length);
			check(valid, "session decrypt of own output, payload " + size);
			check(Arrays.equals(Arrays.copyOf(plain, size), payload), "session round trip, payload " + size);

			// A corrupted packet: decrypt returns false and leaves the buffer untouched.
			final byte[] corrupted = encrypted.clone();
			corrupted[corrupted.length - 1] ^= 0x01;
			final byte[] corruptedAfter = corrupted.clone();
			final boolean corruptedValid = decryptor.decrypt(ReadBuffer.of(ByteBuffer.wrap(corruptedAfter).order(ByteOrder.LITTLE_ENDIAN)), 0, corruptedAfter.length);
			check(!corruptedValid, "corrupted packet must fail, payload " + size);
			check(Arrays.equals(corrupted, corruptedAfter), "invalid packet must stay untouched, payload " + size);

			final Map<String, Object> c = Json.obj();
			c.put("payload", payload);
			c.put("encrypted", encrypted);
			c.put("decrypted", plain);
			c.put("decrypt_valid", true);
			c.put("corrupted", corrupted);
			c.put("corrupted_valid", false);
			session.add(c);
		}

		final Map<String, Object> root = Json.header("org.l2jmobius.loginserver.network.LoginEncryption#encrypt/decrypt");
		root.put("note", "first_packet: the first encrypt() call uses the static key and Rnd.nextInt() as XOR key. Rnd can't be injected, so output = the same steps replayed with the given xor_key; for every case the generator also ran the real class, recovered its random key from the output and checked that the replay is identical. Encrypted size = ((payload + 8) rounded up to the next multiple of 8, always at least +1 byte) + 8. session: after setKey() and one spent first call, encrypt = appendChecksum + session-key crypt; encrypted size = (payload + 4 rounded up the same way) + 8. 'decrypted' is the full buffer after decrypt(): payload, zero padding and the 4-byte checksum slot. A corrupted packet makes decrypt() return false and leaves the buffer untouched.");
		root.put("static_key", staticKey);
		root.put("session_key", sessionKey);
		root.put("first_packet", first);
		root.put("session", session);
		Json.write(outDir.resolve("crypto_login_encryption.json"), root);
	}

	// ------------------------------------------------------------------------------------------------
	// crypto_scrambled_modulus.json
	// ------------------------------------------------------------------------------------------------

	private static final class Rsa
	{
		final BigInteger n;
		final BigInteger e;
		final BigInteger d;
		final RSAPublicKey pub;
		final RSAPrivateKey priv;

		Rsa(BigInteger n, BigInteger e, BigInteger d) throws Exception
		{
			this.n = n;
			this.e = e;
			this.d = d;
			final KeyFactory factory = KeyFactory.getInstance("RSA");
			pub = (RSAPublicKey) factory.generatePublic(new RSAPublicKeySpec(n, e));
			priv = (RSAPrivateKey) factory.generatePrivate(new RSAPrivateKeySpec(n, d));
		}
	}

	/** Deterministic RSA key with e = 65537 (like {@code RSAKeyGenParameterSpec.F4} in the servers). */
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
			return new Rsa(n, e, e.modInverse(phi));
		}
	}

	/** Big-endian bytes of a positive number without the sign byte. */
	private static byte[] unsigned(BigInteger value)
	{
		final byte[] b = value.toByteArray();
		return b[0] == 0 ? Arrays.copyOfRange(b, 1, b.length) : b;
	}

	private static Map<String, Object> scrambleCase(BigInteger modulus, String what) throws Exception
	{
		final RSAPublicKey pub = (RSAPublicKey) KeyFactory.getInstance("RSA").generatePublic(new RSAPublicKeySpec(modulus, BigInteger.valueOf(65537)));
		final RSAPrivateKey priv = (RSAPrivateKey) KeyFactory.getInstance("RSA").generatePrivate(new RSAPrivateKeySpec(modulus, BigInteger.valueOf(3)));
		final ScrambledKeyPair pair = new ScrambledKeyPair(new KeyPair(pub, priv));
		final Map<String, Object> c = Json.obj();
		c.put("what", what);
		c.put("modulus", unsigned(modulus));
		c.put("to_byte_array_length", modulus.toByteArray().length);
		c.put("scrambled", pair.getScrambledModulus());
		return c;
	}

	private static void scrambledModulus(Path outDir) throws Exception
	{
		final Random random = new Random(0x4C320004L);
		final List<Object> cases = new ArrayList<>();

		// A real RSA-1024 modulus (also the key used in crypto_rsa_nopadding.json).
		cases.add(scrambleCase(rsaKey(1024, 0x4C320005L).n, "RSA-1024 key from crypto_rsa_nopadding.json; top bit set, toByteArray() has a leading 0x00"));
		for (int i = 0; i < 3; i++)
		{
			cases.add(scrambleCase(new BigInteger(1024, random).setBit(1023).setBit(0), "random 1024-bit odd number, top bit set; toByteArray() has a leading 0x00"));
		}
		// Top bit clear: toByteArray() is 128 bytes with no leading 0x00 (Java then skips the strip step).
		cases.add(scrambleCase(new BigInteger(1020, random).setBit(1019).setBit(0), "1020-bit odd number; toByteArray() is 128 bytes, no leading 0x00"));
		cases.add(scrambleCase(new BigInteger(1017, random).setBit(1016).setBit(0), "1017-bit odd number; toByteArray() is 128 bytes, no leading 0x00"));

		final Map<String, Object> root = Json.header("org.l2jmobius.loginserver.controller.ScrambledKeyPair#scrambleModulus");
		root.put("note", "modulus = big-endian unsigned bytes (no sign byte). scrambled = getScrambledModulus() (128 bytes). Moduli other than the first are not valid RSA moduli (not products of two primes), they only exercise the byte shuffling. Moduli shorter than 1017 bits make Java throw ArrayIndexOutOfBounds and are not covered.");
		root.put("cases", cases);
		Json.write(outDir.resolve("crypto_scrambled_modulus.json"), root);
	}

	// ------------------------------------------------------------------------------------------------
	// crypto_rsa_nopadding.json
	// ------------------------------------------------------------------------------------------------

	private static byte[] rsaNoPadding(int mode, java.security.Key key, byte[] input) throws Exception
	{
		final Cipher cipher = Cipher.getInstance("RSA/ECB/nopadding");
		cipher.init(mode, key);
		return cipher.doFinal(input);
	}

	private static List<Object> rsaCases(Rsa key, int length, Random random, boolean includeShort) throws Exception
	{
		final List<Object> cases = new ArrayList<>();
		final List<byte[]> plains = new ArrayList<>();
		for (int i = 0; i < 3; i++)
		{
			final byte[] plain = bytes(random, length);
			plain[0] &= 0x3F; // Must stay below the modulus (its top bit is set).
			plains.add(plain);
		}
		final byte[] low = new byte[length];
		low[length - 1] = 1;
		plains.add(low);
		if (includeShort)
		{
			// Shorter than the key: the cipher left-pads with zeros (the Blowfish key sent by the game server is 22 bytes).
			plains.add(bytes(random, 22));
		}
		for (byte[] plain : plains)
		{
			final byte[] cipher = rsaNoPadding(Cipher.ENCRYPT_MODE, key.pub, plain);
			final byte[] back = rsaNoPadding(Cipher.DECRYPT_MODE, key.priv, cipher);
			final Map<String, Object> c = Json.obj();
			c.put("plain", plain);
			c.put("encrypted_with_public", cipher);
			c.put("decrypted_with_private", back);
			cases.add(c);
		}

		// Ciphertexts that weren't produced by us (the other direction of the API; value must be below the modulus).
		for (int i = 0; i < 2; i++)
		{
			final byte[] cipher = bytes(random, length);
			cipher[0] &= 0x3F;
			final Map<String, Object> c = Json.obj();
			c.put("plain", null);
			c.put("encrypted_with_public", cipher);
			c.put("decrypted_with_private", rsaNoPadding(Cipher.DECRYPT_MODE, key.priv, cipher));
			cases.add(c);
		}
		return cases;
	}

	private static Map<String, Object> rsaFile(String what, Rsa key, List<Object> cases)
	{
		final Map<String, Object> o = Json.obj();
		o.put("what", what);
		o.put("n", unsigned(key.n));
		o.put("e", unsigned(key.e));
		o.put("d", unsigned(key.d));
		o.put("cases", cases);
		return o;
	}

	private static void rsaNoPadding(Path outDir) throws Exception
	{
		final Random random = new Random(0x4C320006L);
		final Rsa rsa1024 = rsaKey(1024, 0x4C320005L);
		final Rsa rsa512 = rsaKey(512, 0x4C320007L);

		final Map<String, Object> root = Json.header("javax.crypto.Cipher RSA/ECB/nopadding (as used by LoginController, RequestAuthLogin, gameserverpackets BlowFishKey)");
		root.put("note", "Throwaway test keys generated from a fixed seed, never used anywhere else. n, e, d are big-endian hex without a sign byte. 'plain' null = a ciphertext chosen by the generator, only the decrypt direction is defined. Plaintext is left-padded to the key size by Java; the 22-byte plain case matches the GS->LS Blowfish key packet. decrypted_with_private is always the full key-size output (leading zeros kept).");
		root.put("rsa_1024", rsaFile("client credentials block, LoginServer key", rsa1024, rsaCases(rsa1024, 128, random, false)));
		root.put("rsa_512", rsaFile("GS<->LS link key (RSAKeyGenParameterSpec(512, F4))", rsa512, rsaCases(rsa512, 64, random, true)));
		Json.write(outDir.resolve("crypto_rsa_nopadding.json"), root);
	}

	// ------------------------------------------------------------------------------------------------
	// crypto_game_xor.json
	// ------------------------------------------------------------------------------------------------

	private static Map<String, Object> gameCall(boolean enabledBefore, byte[] input, byte[] output, byte[] keyAfter)
	{
		final Map<String, Object> c = Json.obj();
		c.put("enabled_before", enabledBefore);
		c.put("input", input);
		c.put("output", output);
		c.put("key_after", keyAfter);
		return c;
	}

	private static Map<String, Object> gameSequence(String what, byte[] key, int[] sizes, Random random) throws Exception
	{
		// Encrypt side. Call 1 only enables the cipher (data and key stay as they are).
		final Encryption encrypt = new Encryption();
		encrypt.setKey(key);
		final List<Object> encryptCalls = new ArrayList<>();
		final List<byte[]> plains = new ArrayList<>();
		final List<byte[]> ciphers = new ArrayList<>();

		final byte[] enablePacket = bytes(random, 6);
		final byte[] enableOut = gameEncrypt(encrypt, enablePacket);
		check(Arrays.equals(enablePacket, enableOut), "first encrypt must not change data");
		check(Arrays.equals(key, getInstanceBytes(encrypt, "_outKey")), "first encrypt must not advance the key");
		encryptCalls.add(gameCall(false, enablePacket, enableOut, getInstanceBytes(encrypt, "_outKey")));

		for (int size : sizes)
		{
			final byte[] plain = bytes(random, size);
			final byte[] cipher = gameEncrypt(encrypt, plain);
			plains.add(plain);
			ciphers.add(cipher);
			encryptCalls.add(gameCall(true, plain, cipher, getInstanceBytes(encrypt, "_outKey")));
		}

		// Decrypt side: a separate instance. Before it is enabled, decrypt does nothing.
		final Encryption decrypt = new Encryption();
		decrypt.setKey(key);
		final List<Object> decryptCalls = new ArrayList<>();
		final byte[] early = bytes(random, 9);
		final byte[] earlyOut = gameDecrypt(decrypt, early);
		check(Arrays.equals(early, earlyOut), "decrypt before enabled must not change data");
		check(Arrays.equals(key, getInstanceBytes(decrypt, "_inKey")), "decrypt before enabled must not advance the key");
		decryptCalls.add(gameCall(false, early, earlyOut, getInstanceBytes(decrypt, "_inKey")));
		gameEncrypt(decrypt, new byte[]
		{
			0
		}); // Enables the instance (only the encrypt side advances _outKey, never _inKey).

		for (int i = 0; i < ciphers.size(); i++)
		{
			final byte[] output = gameDecrypt(decrypt, ciphers.get(i));
			check(Arrays.equals(output, plains.get(i)), "game xor round trip, packet " + i);
			decryptCalls.add(gameCall(true, ciphers.get(i), output, getInstanceBytes(decrypt, "_inKey")));
		}

		final Map<String, Object> o = Json.obj();
		o.put("what", what);
		o.put("key", key);
		o.put("encrypt_calls", encryptCalls);
		o.put("decrypt_calls", decryptCalls);
		return o;
	}

	private static void gameXor(Path outDir) throws Exception
	{
		final Random random = new Random(0x4C320008L);

		final byte[] key = bytes(random, 16);
		// Rolling offset at key[8..11] starts at 0xFFFFFFFE, so it wraps on the second packet.
		final byte[] wrapKey = bytes(random, 16);
		wrapKey[8] = (byte) 0xFE;
		wrapKey[9] = (byte) 0xFF;
		wrapKey[10] = (byte) 0xFF;
		wrapKey[11] = (byte) 0xFF;

		final List<Object> sequences = new ArrayList<>();
		sequences.add(gameSequence("random key, 5 packets", key, new int[]
		{
			1,
			7,
			16,
			17,
			40
		}, random));
		sequences.add(gameSequence("key whose rolling offset wraps past 2^32, 6 packets (one empty)", wrapKey, new int[]
		{
			3,
			20,
			0,
			1,
			33,
			16
		}, random));

		final byte[] generated = BlowFishKeygen.getRandomKey();

		final Map<String, Object> root = Json.header("org.l2jmobius.gameserver.network.Encryption#setKey/encrypt/decrypt, BlowFishKeygen");
		root.put("note", "Per sequence: encrypt_calls run on one Encryption instance (the first call only enables the cipher), decrypt_calls on another (decrypt does nothing until enabled; the generator enabled it with a 1-byte encrypt call between the first and second decrypt call). key_after = the instance's inbound/outbound 16-byte key after the call (bytes 8..11 hold the rolling little-endian offset, advanced by the packet size). A zero-length packet changes nothing. blowfish_keygen: BlowFishKeygen.getRandomKey() is 16 bytes = 8 random + this fixed 8-byte tail.");
		root.put("sequences", sequences);
		final Map<String, Object> keygen = Json.obj();
		keygen.put("length", generated.length);
		keygen.put("fixed_tail", Arrays.copyOfRange(generated, 8, 16));
		root.put("blowfish_keygen", keygen);
		Json.write(outDir.resolve("crypto_game_xor.json"), root);
	}
}
