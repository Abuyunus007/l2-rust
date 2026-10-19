package org.l2jmobius.commons.util;

import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.ThreadLocalRandom;

/**
 * Java-vectors harness copy of {@code org.l2jmobius.commons.util.Rnd} (same public API, same range rules).<br>
 * It sits on the class path before the L2JMobius sources, so the real stat and formula code runs unchanged but every draw can come from a script
 * instead of {@code ThreadLocalRandom}. The Java tree is not modified.
 * <p>
 * Every scripted draw is a unit value {@code u} in [0, 1). The scripted list holds those unit values; the range methods map them like this:
 * <ul>
 * <li>{@code get(bound)} = {@code (int) (u * bound)} (when {@code bound <= 0} the result is 0 as in the original, and no value is consumed or recorded)</li>
 * <li>{@code get(origin, bound)} = {@code origin + (int) (u * (bound - origin + 1))}, both ends inclusive (when {@code origin >= bound} the result is the origin as in the original, and no value is consumed or recorded)</li>
 * <li>{@code nextDouble()} = {@code u}, {@code nextBoolean()} = {@code u >= 0.5}</li>
 * </ul>
 * While a script is armed ({@link #arm(double[])}), the arming thread's draws are consumed from it and recorded (method, arguments, returned value). When no script is armed,
 * or on any other thread, every method does exactly what the original does ({@code ThreadLocalRandom}), so the other generators are not affected by this class.
 */
public class Rnd
{
	private static final int MINIMUM_POSITIVE_INT = 1;
	private static final long MINIMUM_POSITIVE_LONG = 1L;
	private static final float MINIMUM_POSITIVE_FLOAT = Float.intBitsToFloat(0x1);
	private static final double MINIMUM_POSITIVE_DOUBLE = Double.longBitsToDouble(0x1L);

	/** One recorded draw. */
	public static final class Call
	{
		public final String method;
		public final String bound;
		public final String returned;

		Call(String method, String bound, String returned)
		{
			this.method = method;
			this.bound = bound;
			this.returned = returned;
		}
	}

	private static final Object LOCK = new Object();

	private static Thread _owner = null;
	private static double[] _script = null;
	private static int _next = 0;
	private static List<Call> _calls = null;

	/**
	 * Arms the script for the current thread.
	 * @param script the unit values, each in [0, 1)
	 */
	public static void arm(double[] script)
	{
		synchronized (LOCK)
		{
			_owner = Thread.currentThread();
			_script = script.clone();
			_next = 0;
			_calls = new ArrayList<>();
		}
	}

	/**
	 * Disarms the script.
	 * @return the draws recorded since {@link #arm(double[])}, in order
	 */
	public static List<Call> disarm()
	{
		synchronized (LOCK)
		{
			final List<Call> calls = _calls == null ? new ArrayList<>() : _calls;
			_owner = null;
			_script = null;
			_calls = null;
			_next = 0;
			return calls;
		}
	}

	/**
	 * @return how many scripted values were consumed since {@link #arm(double[])}
	 */
	public static int used()
	{
		synchronized (LOCK)
		{
			return _next;
		}
	}

	private static boolean scripted()
	{
		synchronized (LOCK)
		{
			return (_owner == Thread.currentThread()) && (_script != null);
		}
	}

	/** Next scripted unit value; call only while {@link #scripted()} (holding no lock is fine, only the arming thread reaches this). */
	private static double unit()
	{
		synchronized (LOCK)
		{
			if (_next >= _script.length)
			{
				throw new IllegalStateException("Rnd script exhausted after " + _next + " draws");
			}

			return _script[_next++];
		}
	}

	private static void record(String method, String bound, Object returned)
	{
		synchronized (LOCK)
		{
			_calls.add(new Call(method, bound, String.valueOf(returned)));
		}
	}

	/**
	 * @return a random boolean value.
	 */
	public static boolean nextBoolean()
	{
		if (scripted())
		{
			final boolean value = unit() >= 0.5;
			record("nextBoolean", "", value);
			return value;
		}

		return ThreadLocalRandom.current().nextBoolean();
	}

	/**
	 * Generates random bytes and places them into a user-supplied byte array. The number of random bytes produced is equal to the length of the byte array.
	 * @param bytes the byte array to fill with random bytes.
	 */
	public static void nextBytes(byte[] bytes)
	{
		if (scripted())
		{
			final StringBuilder sb = new StringBuilder();
			for (int i = 0; i < bytes.length; i++)
			{
				bytes[i] = (byte) (int) (unit() * 256);
				sb.append(bytes[i]).append(i + 1 < bytes.length ? "," : "");
			}

			record("nextBytes", Integer.toString(bytes.length), sb);
			return;
		}

		ThreadLocalRandom.current().nextBytes(bytes);
	}

	/**
	 * @param bound (int)
	 * @return a random int value between zero (inclusive) and the specified bound (exclusive).
	 */
	public static int get(int bound)
	{
		if (scripted())
		{
			if (bound <= 0)
			{
				return 0; // No draw, like the original.
			}

			final int value = (int) (unit() * bound);
			record("get_int", Integer.toString(bound), value);
			return value;
		}

		return bound <= 0 ? 0 : ThreadLocalRandom.current().nextInt(bound);
	}

	/**
	 * @param origin (int)
	 * @param bound (int)
	 * @return a random int value between the specified origin (inclusive) and the specified bound (inclusive).
	 */
	public static int get(int origin, int bound)
	{
		if (scripted())
		{
			if (origin >= bound)
			{
				return origin; // No draw, like the original.
			}

			final int value = (int) (origin + (long) (unit() * (((long) bound - origin) + 1)));
			record("get_int_range", origin + "," + bound, value);
			return value;
		}

		return origin >= bound ? origin : ThreadLocalRandom.current().nextInt(origin, bound == Integer.MAX_VALUE ? bound : bound + MINIMUM_POSITIVE_INT);
	}

	/**
	 * @return a random int value.
	 */
	public static int nextInt()
	{
		if (scripted())
		{
			final int value = (int) (long) ((unit() * 4294967296.0) - 2147483648.0);
			record("nextInt", "", value);
			return value;
		}

		return ThreadLocalRandom.current().nextInt();
	}

	/**
	 * @param bound (long)
	 * @return a random long value between zero (inclusive) and the specified bound (exclusive).
	 */
	public static long get(long bound)
	{
		if (scripted())
		{
			if (bound <= 0)
			{
				return 0;
			}

			final long value = (long) (unit() * bound);
			record("get_long", Long.toString(bound), value);
			return value;
		}

		return bound <= 0 ? 0 : ThreadLocalRandom.current().nextLong(bound);
	}

	/**
	 * @param origin (long)
	 * @param bound (long)
	 * @return a random long value between the specified origin (inclusive) and the specified bound (inclusive).
	 */
	public static long get(long origin, long bound)
	{
		if (scripted())
		{
			if (origin >= bound)
			{
				return origin;
			}

			final long value = origin + (long) (unit() * (((double) bound - origin) + 1));
			record("get_long_range", origin + "," + bound, value);
			return value;
		}

		return origin >= bound ? origin : ThreadLocalRandom.current().nextLong(origin, bound == Long.MAX_VALUE ? bound : bound + MINIMUM_POSITIVE_LONG);
	}

	/**
	 * @return a random long value.
	 */
	public static long nextLong()
	{
		if (scripted())
		{
			final long value = (long) ((unit() - 0.5) * 1.8446744073709552E19);
			record("nextLong", "", value);
			return value;
		}

		return ThreadLocalRandom.current().nextLong();
	}

	/**
	 * @param bound (float)
	 * @return a random float value between zero (inclusive) and the specified bound (exclusive).
	 */
	public static float get(float bound)
	{
		if (scripted())
		{
			if (bound <= 0)
			{
				return 0;
			}

			final float value = (float) (unit() * bound);
			record("get_float", Float.toString(bound), value);
			return value;
		}

		return bound <= 0 ? 0 : ThreadLocalRandom.current().nextFloat(bound);
	}

	/**
	 * @param origin (float)
	 * @param bound (float)
	 * @return a random float value between the specified origin (inclusive) and the specified bound (inclusive).
	 */
	public static float get(float origin, float bound)
	{
		if (scripted())
		{
			if (origin >= bound)
			{
				return origin;
			}

			final float value = (float) (origin + (unit() * (bound - origin)));
			record("get_float_range", origin + "," + bound, value);
			return value;
		}

		return origin >= bound ? origin : ThreadLocalRandom.current().nextFloat(origin, bound == Float.MAX_VALUE ? bound : bound + MINIMUM_POSITIVE_FLOAT);
	}

	/**
	 * @return a random float value between zero (inclusive) and one (exclusive).
	 */
	public static float nextFloat()
	{
		if (scripted())
		{
			final float value = (float) unit();
			record("nextFloat", "", value);
			return value;
		}

		return ThreadLocalRandom.current().nextFloat();
	}

	/**
	 * @param bound (double)
	 * @return a random double value between zero (inclusive) and the specified bound (exclusive).
	 */
	public static double get(double bound)
	{
		if (scripted())
		{
			if (bound <= 0)
			{
				return 0;
			}

			final double value = unit() * bound;
			record("get_double", Double.toString(bound), value);
			return value;
		}

		return bound <= 0 ? 0 : ThreadLocalRandom.current().nextDouble(bound);
	}

	/**
	 * @param origin (double)
	 * @param bound (double)
	 * @return a random double value between the specified origin (inclusive) and the specified bound (inclusive).
	 */
	public static double get(double origin, double bound)
	{
		if (scripted())
		{
			if (origin >= bound)
			{
				return origin;
			}

			final double value = origin + (unit() * (bound - origin));
			record("get_double_range", origin + "," + bound, value);
			return value;
		}

		return origin >= bound ? origin : ThreadLocalRandom.current().nextDouble(origin, bound == Double.MAX_VALUE ? bound : bound + MINIMUM_POSITIVE_DOUBLE);
	}

	/**
	 * @return a random double value between zero (inclusive) and one (exclusive).
	 */
	public static double nextDouble()
	{
		if (scripted())
		{
			final double value = unit();
			record("nextDouble", "", value);
			return value;
		}

		return ThreadLocalRandom.current().nextDouble();
	}

	/**
	 * @return the next random, Gaussian ("normally") distributed double value with mean 0.0 and standard deviation 1.0 from this random number generator's sequence.
	 */
	public static double nextGaussian()
	{
		if (scripted())
		{
			// Not used by the stat or formula vectors; a deterministic stand-in so a script still replays.
			final double value = (unit() * 2.0) - 1.0;
			record("nextGaussian", "", value);
			return value;
		}

		return ThreadLocalRandom.current().nextGaussian();
	}
}
