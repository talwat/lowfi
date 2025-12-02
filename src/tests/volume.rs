#[cfg(test)]
mod volume {
    use crate::volume::PersistentVolume;

    #[test]
    fn float_converts_percent() {
        let pv = PersistentVolume { inner: 75 };
        assert!((pv.float() - 0.75).abs() < f32::EPSILON);
    }

    #[test]
    fn float_zero_volume() {
        let pv = PersistentVolume { inner: 0 };
        assert_eq!(pv.float(), 0.0);
    }

    #[test]
    fn float_full_volume() {
        let pv = PersistentVolume { inner: 100 };
        assert_eq!(pv.float(), 1.0);
    }

    #[test]
    fn float_mid_range() {
        let pv = PersistentVolume { inner: 50 };
        assert!((pv.float() - 0.5).abs() < f32::EPSILON);
    }
}
